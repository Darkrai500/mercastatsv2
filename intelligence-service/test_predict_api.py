"""Pruebas del endpoint POST /predict/next (contrato con el backend Rust)."""

from datetime import datetime, timedelta

import pytest
from fastapi.testclient import TestClient
from loguru import logger

import main

client = TestClient(main.app)

MONDAY = datetime(2026, 8, 3)  # lunes


def history(weeks: int = 10, hour: int = 18, total: float = 50.0) -> list[dict]:
    """Lunes y jueves a la misma hora, con los campos mínimos que envía el backend."""
    rows = []
    for week in range(weeks):
        for weekday in (0, 3):
            when = (MONDAY + timedelta(weeks=week, days=weekday)).replace(hour=hour)
            rows.append({"fecha_hora": when.isoformat(), "total": total})
    return rows


def last_thursday(rows: list[dict]) -> datetime:
    return datetime.fromisoformat(rows[-1]["fecha_hora"])


def post(body: dict):
    return client.post("/predict/next", json=body)


class TestContrato:
    def test_forma_de_la_respuesta(self):
        rows = history()
        now = last_thursday(rows) + timedelta(hours=4)
        response = post({"current_date": now.isoformat(), "history_features": rows})

        assert response.status_code == 200
        prediction = response.json()["prediction"]
        assert set(prediction) == {
            "timestamp", "time_window_label", "time_window_range", "day_label",
            "estimated_total", "estimated_total_min", "estimated_total_max",
            "confidence", "suggested_products", "learning_mode",
        }
        assert prediction["learning_mode"] is False
        assert prediction["estimated_total_min"] <= prediction["estimated_total"] <= prediction["estimated_total_max"]
        assert 0.0 <= prediction["confidence"] <= 1.0
        assert prediction["time_window_range"] == "17:00 - 19:00"  # ±1 h en torno a las 18:00
        # El frontend parsea el instante como DateTime<FixedOffset>: debe llevar offset.
        assert datetime.fromisoformat(prediction["timestamp"]).utcoffset() is not None

    def test_el_dia_previsto_es_el_lunes_y_se_describe_en_espanol(self):
        rows = history()
        thursday = last_thursday(rows)
        now = thursday + timedelta(hours=4)  # jueves por la noche
        prediction = post({"current_date": now.isoformat(), "history_features": rows}).json()["prediction"]

        monday = thursday + timedelta(days=4)
        assert prediction["timestamp"].startswith(monday.date().isoformat() + "T18:00")
        assert prediction["day_label"] == "el próximo lunes"
        assert prediction["time_window_label"] == "el próximo lunes, alrededor de las 18:00"

    @pytest.mark.parametrize(
        "days_ahead,expected",
        [(0, "hoy (lunes)"), (1, "mañana (lunes)")],
    )
    def test_etiquetas_hoy_y_manana(self, days_ahead, expected):
        rows = history()
        monday = MONDAY + timedelta(weeks=10)
        now = (monday - timedelta(days=days_ahead)).replace(hour=9)
        prediction = post({"current_date": now.isoformat(), "history_features": rows}).json()["prediction"]
        assert prediction["day_label"] == expected

    def test_campos_heredados_se_aceptan_e_ignoran(self):
        """El backend anterior enviaba características derivadas (con DOW de SQL, Rust y pandas
        en convenciones distintas). Ahora se ignoran: todo se deriva de fecha_hora."""
        rows = history()
        for row in rows:
            row.update(
                numero_factura="0000-000-000000", day_of_week=99, day_of_month=99, hour_of_day=99,
                days_since_last_shop=-5.0, total_last_30d=1e9, tickets_last_30d=-1, is_payday_week=True,
            )
        body = {
            "user_id": "alguien@example.com",
            "current_date": (last_thursday(rows) + timedelta(hours=4)).isoformat(),
            "features_now": {"day_of_week": 99, "hour_of_day": 99},
            "history_features": rows,
        }
        response = post(body)
        assert response.status_code == 200
        assert response.json()["prediction"]["day_label"] == "el próximo lunes"

    def test_filas_invalidas_se_descartan(self):
        rows = history() + [
            {"fecha_hora": "no-es-una-fecha", "total": 10.0},
            {"fecha_hora": None, "total": 10.0},
            {"fecha_hora": "2026-09-01T10:00:00", "total": None},
            {"fecha_hora": "2026-09-01T10:00:00", "total": -3.0},
        ]
        now = last_thursday(history()) + timedelta(hours=4)
        assert post({"current_date": now.isoformat(), "history_features": rows}).status_code == 200

    def test_fecha_actual_invalida_es_422_sin_repetir_lo_recibido(self):
        response = post({"current_date": "VALOR-PRIVADO", "history_features": history()})
        assert response.status_code == 422
        assert "VALOR-PRIVADO" not in response.text
        assert response.json()["detail"][0]["loc"] == ["body", "current_date"]


class TestSinDatosSuficientes:
    @pytest.mark.parametrize("n", [0, 1, 4])
    def test_responde_200_en_modo_aprendizaje(self, n):
        """Antes: 503 sin historial y 500 con 1-6 tickets (excepciones de scikit-learn)."""
        rows = history(weeks=3)[:n]
        response = post({"current_date": "2026-09-01T10:00:00", "history_features": rows})
        assert response.status_code == 200
        prediction = response.json()["prediction"]
        assert prediction["learning_mode"] is True
        assert prediction["confidence"] == 0.0

    def test_sin_el_campo_de_historial(self):
        assert post({"current_date": "2026-09-01T10:00:00"}).json()["prediction"]["learning_mode"] is True


class TestZonaHoraria:
    def test_current_date_en_utc_se_convierte_a_hora_local(self):
        """23:30 UTC del sábado son las 01:30 del domingo en Madrid (verano): el 'ahora' local
        ya es domingo. Antes se usaba la hora UTC como si fuera local."""
        rows = history()
        saturday_utc = datetime(2026, 10, 17, 23, 30)  # sábado, UTC; en Madrid ya es domingo (+02:00)
        body = {"current_date": saturday_utc.isoformat() + "+00:00", "history_features": rows}
        prediction = post(body).json()["prediction"]
        # Desde el punto de vista local ya es domingo 18 → el lunes siguiente es "mañana".
        assert prediction["day_label"] == "mañana (lunes)"

    def test_fecha_con_nanosegundos_como_la_que_envia_chrono(self):
        rows = history()
        body = {"current_date": "2026-10-11T20:22:00.123456789+00:00", "history_features": rows}
        assert post(body).status_code == 200

    @pytest.mark.parametrize(
        "last_visit,offset",
        [(datetime(2026, 7, 9, 18), "+02:00"), (datetime(2026, 1, 8, 18), "+01:00")],
    )
    def test_el_offset_respeta_el_horario_de_verano(self, last_visit, offset):
        rows = [
            {"fecha_hora": (last_visit - timedelta(days=3 * (59 - i))).isoformat(), "total": 40.0}
            for i in range(60)
        ]
        now = (last_visit + timedelta(hours=14)).isoformat()
        prediction = post({"current_date": now, "history_features": rows}).json()["prediction"]
        assert prediction["timestamp"].endswith(offset)

    def test_zona_configurable(self, monkeypatch):
        monkeypatch.setenv("APP_TIMEZONE", "Atlantic/Canary")
        rows = history()
        now = last_thursday(rows) + timedelta(hours=4)
        prediction = post({"current_date": now.isoformat(), "history_features": rows}).json()["prediction"]
        assert prediction["timestamp"].endswith(("+00:00", "+01:00"))  # Canarias: UTC / UTC+1


class TestProductos:
    def test_las_sugerencias_llegan_en_el_formato_del_contrato(self):
        rows = history(weeks=12)
        thursday = last_thursday(rows)
        lines = []
        for row in rows:
            lines.append({"name": "PAN BARRA", "fecha_hora": row["fecha_hora"], "quantity": 1, "unit_price": 0.8})
        body = {
            "current_date": (thursday + timedelta(hours=4)).isoformat(),
            "history_features": rows,
            "product_history": lines,
        }
        products = post(body).json()["prediction"]["suggested_products"]
        assert [p["name"] for p in products] == ["PAN BARRA"]
        assert set(products[0]) == {"name", "probability", "price_estimation", "reason"}
        assert products[0]["price_estimation"] == pytest.approx(0.8)

    def test_lineas_invalidas_se_descartan(self):
        rows = history()
        body = {
            "current_date": (last_thursday(rows) + timedelta(hours=4)).isoformat(),
            "history_features": rows,
            "product_history": [
                {"name": "", "fecha_hora": rows[0]["fecha_hora"], "quantity": 1, "unit_price": 1},
                {"name": "X", "fecha_hora": "mal", "quantity": 1, "unit_price": 1},
                {"name": "X", "fecha_hora": rows[0]["fecha_hora"], "quantity": 0, "unit_price": 1},
            ],
        }
        response = post(body)
        assert response.status_code == 200
        assert response.json()["prediction"]["suggested_products"] == []


class TestPrivacidad:
    def test_los_logs_no_contienen_datos_del_usuario(self):
        messages: list[str] = []
        sink_id = logger.add(lambda message: messages.append(str(message)), level="DEBUG")
        try:
            rows = history()
            for row in rows:
                row["numero_factura"] = "FACTURA-SECRETA-123"
            body = {
                "user_id": "persona.privada@example.com",
                "current_date": (last_thursday(rows) + timedelta(hours=4)).isoformat(),
                "history_features": rows,
                "product_history": [
                    {"name": "PRODUCTO-PRIVADO", "fecha_hora": rows[0]["fecha_hora"], "quantity": 1, "unit_price": 12.34}
                ],
            }
            assert post(body).status_code == 200
        finally:
            logger.remove(sink_id)

        logged = "\n".join(messages)
        assert logged, "se esperaba al menos una línea de log"
        for secret in ("persona.privada", "FACTURA-SECRETA", "PRODUCTO-PRIVADO", "12.34", "50.0"):
            assert secret not in logged


class TestAutenticacionInterna:
    def test_con_clave_configurada_se_exige(self, monkeypatch):
        monkeypatch.setenv("INTELLIGENCE_API_KEY", "clave-de-prueba")
        body = {"current_date": "2026-09-01T10:00:00", "history_features": []}
        assert post(body).status_code == 401
        ok = client.post("/predict/next", json=body, headers={"X-API-Key": "clave-de-prueba"})
        assert ok.status_code == 200
