"""
Pruebas del predictor de próxima compra (``predictor.py``).

Cada bloque protege un fallo real del predictor anterior (RandomForest/KNN):
caídas con pocos tickets, fecha anclada en "ahora", hora ajena al día de la semana,
estado compartido entre usuarios, etc.
"""

from datetime import date, datetime, timedelta

import pytest

import backtest
from predictor import (
    MIN_VISITS_FOR_PREDICTION,
    NextVisitModel,
    ProductLine,
    Ticket,
    estimate_total,
    group_visits,
    predict_next_purchase,
    suggest_products,
)

MONDAY = datetime(2026, 8, 3)  # lunes


def at(day: datetime, hour: float) -> datetime:
    return day.replace(hour=int(hour), minute=int(round((hour % 1) * 60)))


def weekly_pattern(weekday_hours: dict[int, float], weeks: int, total=50.0) -> list[Ticket]:
    """Compras exactas los días indicados de cada semana (lunes=0), desde MONDAY."""
    tickets = []
    for week in range(weeks):
        for weekday, hour in sorted(weekday_hours.items()):
            day = MONDAY + timedelta(weeks=week, days=weekday)
            tickets.append(Ticket(at(day, hour), total))
    return tickets


# ----------------------------------------------------------------------------
# Visitas
# ----------------------------------------------------------------------------


class TestGroupVisits:
    def test_tickets_del_mismo_dia_son_una_visita(self):
        tickets = [
            Ticket(datetime(2026, 8, 3, 10, 0), 5.0),
            Ticket(datetime(2026, 8, 3, 18, 30), 40.0),
            Ticket(datetime(2026, 8, 4, 12, 0), 7.0),
        ]
        visits = group_visits(tickets)
        assert [v.day for v in visits] == [date(2026, 8, 3), date(2026, 8, 4)]
        assert visits[0].total == pytest.approx(45.0)
        assert visits[0].hour == pytest.approx(18.5)  # hora del ticket principal

    def test_ordena_cronologicamente(self):
        tickets = [Ticket(datetime(2026, 8, 5, 9, 0), 1.0), Ticket(datetime(2026, 8, 3, 9, 0), 1.0)]
        assert [v.day.day for v in group_visits(tickets)] == [3, 5]


# ----------------------------------------------------------------------------
# Cuándo
# ----------------------------------------------------------------------------


class TestNextVisit:
    def test_tras_un_jueves_viene_un_lunes_sin_importar_cuando_se_pregunte(self):
        """Antes la fecha se anclaba en 'ahora' y se deslizaba cada día (mar → mié → jue)."""
        tickets = weekly_pattern({0: 18.5, 3: 18.5}, weeks=10)
        model = NextVisitModel(group_visits(tickets))
        last = tickets[-1].when  # jueves
        assert last.weekday() == 3

        for hours_later in (3, 14, 30, 40, 55):  # jueves noche, viernes, sábado, domingo...
            now = last + timedelta(hours=hours_later)
            predicted = model.predict(now)
            assert predicted.day.weekday() == 0, f"ahora={now}"
            assert predicted.day == (last + timedelta(days=4)).date()

    def test_hora_depende_del_dia_de_la_semana(self):
        """Lunes a las 18:00 y jueves a las 11:00: la hora sale del día previsto."""
        tickets = weekly_pattern({0: 18.0, 3: 11.0}, weeks=10)
        model = NextVisitModel(group_visits(tickets))
        assert model.typical_hour(0)[0] == pytest.approx(18.0, abs=0.5)
        assert model.typical_hour(3)[0] == pytest.approx(11.0, abs=0.5)

    def test_la_hora_habitual_no_cae_entre_dos_picos(self):
        """Con horas 11 y 18 la mediana sería 14:30, donde nunca se compra."""
        tickets = weekly_pattern({1: 11.0, 4: 18.0}, weeks=8)
        model = NextVisitModel(group_visits(tickets))
        for weekday, expected in ((1, 11.0), (4, 18.0)):
            assert abs(model.typical_hour(weekday)[0] - expected) <= 0.5

    @pytest.mark.parametrize("visits", [6, 11, 16, 30])
    def test_nunca_predice_domingo_si_no_compra_en_domingo(self, visits):
        """Quien compra a diario salvo domingos y acaba de comprar un sábado: el hueco de un día
        apuntaría al domingo, con la tienda cerrada. El prior semanal por sí solo no lo evita."""
        tickets, day = [], MONDAY
        while len(tickets) < visits:
            if day.weekday() != 6:
                tickets.append(Ticket(at(day, 12.0), 40.0))
            day += timedelta(days=1)
        model = NextVisitModel(group_visits(tickets))
        last = tickets[-1].when
        for hours in range(1, 24 * 6, 5):
            assert model.predict(last + timedelta(hours=hours)).day.weekday() != 6

    def test_hoy_solo_si_la_hora_habitual_no_ha_pasado(self):
        tickets = weekly_pattern({0: 18.0}, weeks=12)  # todos los lunes a las 18:00
        model = NextVisitModel(group_visits(tickets))
        monday = MONDAY + timedelta(weeks=12)
        assert model.predict(at(monday, 9.0)).day == monday.date()  # por la mañana: hoy
        # Pasadas las 18:00 de hoy ya no puede ser "hoy" (el instante estimado estaría en el pasado).
        assert model.predict(at(monday, 20.0)).day > monday.date()

    def test_la_prediccion_es_siempre_futura(self):
        tickets = weekly_pattern({0: 18.0, 3: 18.0}, weeks=6)
        model = NextVisitModel(group_visits(tickets))
        now = datetime(2026, 9, 20, 22, 0)
        assert model.predict(now).day >= now.date()

    def test_usuario_muy_regular_tiene_alta_confianza_y_el_irregular_baja(self):
        regular = predict_next_purchase(
            weekly_pattern({5: 11.0}, weeks=20), [], at(MONDAY + timedelta(weeks=19, days=6), 20)
        )
        sim = backtest.simulate("irregular", 40, seed=3)
        irregular = predict_next_purchase(
            [Ticket(v.when, v.total) for v in sim], [], sim[-1].when + timedelta(hours=6)
        )
        assert regular.confidence > 0.8
        assert irregular.confidence < regular.confidence - 0.2


# ----------------------------------------------------------------------------
# Robustez: lo que antes provocaba errores 500
# ----------------------------------------------------------------------------


class TestRobustez:
    @pytest.mark.parametrize("n", [0, 1, 2, 3, MIN_VISITS_FOR_PREDICTION - 1])
    def test_pocos_tickets_no_fallan_y_entran_en_modo_aprendizaje(self, n):
        tickets = weekly_pattern({0: 18.0, 3: 18.0}, weeks=5)[:n]
        result = predict_next_purchase(tickets, [], datetime(2026, 9, 1, 10, 0))
        assert result.learning_mode is True
        assert result.confidence == 0.0
        assert result.visits_used == n

    def test_con_el_minimo_de_visitas_ya_predice(self):
        tickets = weekly_pattern({0: 18.0, 3: 18.0}, weeks=5)[:MIN_VISITS_FOR_PREDICTION]
        result = predict_next_purchase(tickets, [], tickets[-1].when + timedelta(hours=2))
        assert result.learning_mode is False

    def test_sin_estado_entre_usuarios(self):
        """Antes había un modelo global: un usuario sin historial recibía el de otro."""
        user_a = weekly_pattern({0: 18.0, 3: 18.0}, weeks=12, total=80.0)
        now = user_a[-1].when + timedelta(hours=5)

        before = predict_next_purchase(user_a, [], now)
        empty_user = predict_next_purchase([], [], now)
        after = predict_next_purchase(user_a, [], now)

        assert empty_user.learning_mode is True
        assert empty_user.total == 0.0  # nada heredado de A (80 €)
        assert before == after

    def test_varios_tickets_el_mismo_instante(self):
        when = datetime(2026, 8, 3, 18, 0)
        tickets = [Ticket(when, 10.0)] * 6 + weekly_pattern({3: 18.0}, weeks=6)
        predict_next_purchase(tickets, [], datetime(2026, 9, 20, 10, 0))  # no debe lanzar


# ----------------------------------------------------------------------------
# Cuánto
# ----------------------------------------------------------------------------


class TestEstimateTotal:
    def _visits(self, totals, gap_days=3):
        day = date(2026, 1, 1)
        visits = []
        for total in totals:
            visits.append(group_visits([Ticket(datetime.combine(day, datetime.min.time()).replace(hour=12), total)])[0])
            day += timedelta(days=gap_days)
        return visits

    def test_usuario_estable_estima_su_gasto_y_el_rango_lo_contiene(self):
        center, low, high = estimate_total(self._visits([50, 52, 48, 51, 49, 50, 53, 47]), gap_days=3)
        assert center == pytest.approx(50, abs=2)
        assert low <= center <= high
        assert high - low < 20

    def test_resultado_dentro_del_rango_observado_y_no_negativo(self):
        center, low, _ = estimate_total(self._visits([5, 6, 4, 5, 5, 6, 4]), gap_days=60)
        assert 4 <= center <= 6
        assert low >= 0

    def test_la_compra_es_mayor_tras_un_hueco_largo_solo_si_hay_relacion(self):
        # El gasto crece con el hueco previo: gap 1 → 20 €, gap 7 → 80 €...
        day, visits, gaps = date(2026, 1, 1), [], [1, 7, 3, 1, 7, 3, 1, 7, 3, 1, 7, 3, 1, 7, 3]
        for gap in gaps:
            day += timedelta(days=gap)
            total = 20 + 10 * gap
            visits.append(group_visits([Ticket(datetime.combine(day, datetime.min.time()).replace(hour=12), total)])[0])
        short = estimate_total(visits, gap_days=1)[0]
        long = estimate_total(visits, gap_days=7)[0]
        assert long > short + 15

        # Sin relación (totales alternos independientes del hueco) no debe inventarse una.
        flat = self._visits([40, 60] * 8, gap_days=3)
        assert estimate_total(flat, gap_days=1)[0] == pytest.approx(estimate_total(flat, gap_days=9)[0])


# ----------------------------------------------------------------------------
# Qué
# ----------------------------------------------------------------------------


def lines_for(name: str, days: list[date], price=1.0, quantity=1.0, hour=18) -> list[ProductLine]:
    return [ProductLine(name, datetime.combine(d, datetime.min.time()).replace(hour=hour), quantity, price) for d in days]


class TestSuggestProducts:
    START = date(2026, 6, 1)

    def _visit_days(self, every=3, n=30):
        return [self.START + timedelta(days=every * i) for i in range(n)]

    def _basket(self):
        """30 visitas cada 3 días. Leche cada 6 días; pan en todas; chocolate una vez."""
        days = self._visit_days()
        lines = lines_for("PAN", days, price=0.8)
        lines += lines_for("LECHE", days[::2], price=0.95)
        lines += lines_for("CHOCOLATE", days[5:6], price=1.5)
        # una línea mínima por visita para que cuenten como visitas aunque el pan faltara
        return lines, days

    def test_toca_comprar_lo_que_se_agota_y_no_lo_recien_comprado(self):
        lines, days = self._basket()
        last_visit = days[-1]
        # La leche se compró en days[-2] (cadencia 6 días): en la visita siguiente (+3 días)
        # ya han pasado 6 → toca. Si la última compra fue en la última visita, no toca.
        due = {s.name: s.probability for s in suggest_products(lines, last_visit + timedelta(days=3))}
        assert due.get("LECHE", 0) > 0.6

        lines_recent = [l for l in lines if l.name != "LECHE"] + lines_for("LECHE", days[1::2], price=0.95)
        recent = {s.name: s.probability for s in suggest_products(lines_recent, last_visit + timedelta(days=3))}
        assert recent.get("LECHE", 0) < 0.45

    def test_basico_diario_tiene_probabilidad_alta(self):
        lines, days = self._basket()
        suggestions = {s.name: s for s in suggest_products(lines, days[-1] + timedelta(days=3))}
        assert suggestions["PAN"].probability > 0.8

    def test_compra_unica_no_se_sugiere(self):
        lines, days = self._basket()
        names = [s.name for s in suggest_products(lines, days[-1] + timedelta(days=3))]
        assert "CHOCOLATE" not in names

    def test_producto_abandonado_pierde_probabilidad(self):
        days = self._visit_days(n=40)
        lines = lines_for("PAN", days) + lines_for("MERMELADA", days[:6], price=2.0)  # dejó de comprarla
        names = [s.name for s in suggest_products(lines, days[-1] + timedelta(days=3))]
        assert "MERMELADA" not in names

    def test_pocas_visitas_no_hay_sugerencias(self):
        days = self._visit_days(n=2)
        assert suggest_products(lines_for("PAN", days), days[-1] + timedelta(days=3)) == []

    def test_precio_estimado_usa_el_ultimo_precio_y_la_cantidad_tipica(self):
        days = self._visit_days(n=12)
        lines = lines_for("YOGUR", days, price=2.0, quantity=2.0)
        lines.append(ProductLine("YOGUR", datetime.combine(days[-1], datetime.min.time()), 2.0, 2.5))
        by_name = {s.name: s for s in suggest_products(lines, days[-1] + timedelta(days=3))}
        assert by_name["YOGUR"].price_estimation == pytest.approx(5.0)  # 2 uds × 2,50 €

    def test_orden_estable_y_acotado(self):
        days = self._visit_days(n=30)
        lines = []
        for i in range(20):
            lines += lines_for(f"P{i:02d}", days)  # 20 productos igual de habituales
        result = suggest_products(lines, days[-1] + timedelta(days=3))
        assert len(result) == 8
        assert [s.name for s in result] == sorted(s.name for s in result)  # empate → orden alfabético
        assert result == suggest_products(lines, days[-1] + timedelta(days=3))


# ----------------------------------------------------------------------------
# Calidad global (simulación)
# ----------------------------------------------------------------------------


class TestBacktest:
    def test_usuario_lunes_jueves_acierta_el_dia_y_supera_la_referencia(self):
        result = backtest.evaluate("lun_jue", seeds=3, n=50, start_at=12)
        assert result["dia_±1"] >= 0.85
        assert result["dia_exacto"] >= 0.75
        assert result["mae_dias"] < 0.6 * result["mae_dias_base"]
        assert result["mae_hora"] < 1.0

    def test_el_intervalo_de_gasto_cubre_aproximadamente_lo_prometido(self):
        result = backtest.evaluate("lun_jue", seeds=3, n=50, start_at=12)
        assert 0.6 <= result["cobertura_gasto"] <= 0.9

    def test_las_sugerencias_superan_a_los_mas_comprados(self):
        result = backtest.evaluate("lun_mie_vie", seeds=3, n=50, start_at=12)
        assert result["prec_productos"] > result["prec_top_cantidad"]


if __name__ == "__main__":
    pytest.main([__file__, "-v"])
