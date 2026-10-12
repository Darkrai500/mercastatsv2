"""Lectura de tickets en foto: detección de formato, modelo de visión, respaldo y avisos.

Todos los datos son sintéticos y el modelo de visión se simula con un transporte HTTP falso.
"""

import base64
import io
import json

import httpx
import pytest
from fastapi.testclient import TestClient
from PIL import Image

import processor
from main import app
from services import vision_parser
from services.image_parser import adapt_paper_ticket_text, extract_paper_total
from services.pdf_parser import ParsedProduct, ParsedTicket, extract_products, parse_ticket
from services.ticket_checks import check_ticket
from test_ocr_api import PDF

TICKET_JSON = {
    "es_ticket_mercadona": True,
    "numero_factura": "0000-000-000001",
    "fecha": "01/02/2026",
    "hora": "18:30",
    "numero_operacion": "000123",
    "tienda": "MERCADONA, S.A.",
    "direccion": "C/ EJEMPLO 1, 00000 CIUDAD",
    "metodo_pago": "TARJETA BANCARIA",
    "total": 7.09,
    "productos": [
        {"nombre": "LECHE ENTERA", "cantidad": 2, "unidad": "unidad", "precio_unitario": 0.95, "precio_total": 1.9},
        {"nombre": "1 PAN DE MOLDE", "cantidad": 1, "unidad": "unidad", "precio_unitario": None, "precio_total": 1.25},
        {"nombre": "MANZANA", "cantidad": 0.82, "unidad": "kg", "precio_unitario": 2.0, "precio_total": 1.64},
        {"nombre": "CAFÉ MOLIDO", "cantidad": 1, "unidad": "unidad", "precio_unitario": 2.3, "precio_total": 2.3},
    ],
    "iva_desglose": [
        {"porcentaje": 4, "base_imponible": 3.06, "cuota": 0.12},
        {"porcentaje": 10, "base_imponible": 3.56, "cuota": 0.35},
    ],
}


def png_bytes() -> bytes:
    buffer = io.BytesIO()
    Image.new("RGB", (60, 120), "white").save(buffer, format="PNG")
    return buffer.getvalue()


def openrouter_reply(payload) -> httpx.Response:
    content = payload if isinstance(payload, str) else json.dumps(payload)
    return httpx.Response(200, json={"choices": [{"message": {"content": content}}], "usage": {"cost": 0.001}})


@pytest.fixture
def vision(monkeypatch):
    """Configura el lector por visión contra un OpenRouter simulado; devuelve las peticiones."""
    monkeypatch.delenv("INTELLIGENCE_API_KEY", raising=False)
    monkeypatch.setenv("OPENROUTER_API_KEY", "sk-or-prueba")
    monkeypatch.setenv("OCR_VISION_MODELS", "modelo/principal,modelo/respaldo")
    state = {"requests": [], "replies": {}}

    def handler(request: httpx.Request) -> httpx.Response:
        body = json.loads(request.content)
        state["requests"].append(body)
        reply = state["replies"].get(body["model"], openrouter_reply(TICKET_JSON))
        return reply() if callable(reply) else reply

    real_client = httpx.Client
    monkeypatch.setattr(
        vision_parser.httpx,
        "Client",
        lambda **kwargs: real_client(transport=httpx.MockTransport(handler), **kwargs),
    )
    return state


def post_file(data: bytes, file_name: str = "ticket.jpg", mime: str = "image/jpeg"):
    return TestClient(app).post(
        "/ocr/process",
        json={
            "ticket_id": "t1",
            "file_name": file_name,
            "file_content_b64": base64.b64encode(data).decode(),
            "mime_type": mime,
        },
    )


# --------------------------------------------------------------------- formato


@pytest.mark.parametrize(
    ("header", "photo"),
    [
        (b"\xff\xd8\xff\xe0\x00\x10JFIF", True),
        (b"\x89PNG\r\n\x1a\n\x00\x00", True),
        (b"RIFF\x00\x00\x00\x00WEBPVP8 ", True),
        (b"\x00\x00\x00\x18ftypheic\x00\x00", True),
        (b"%PDF-1.7\n", False),
        (b"PK\x03\x04", False),
    ],
)
def test_reconoce_las_fotos_por_su_contenido(header, photo) -> None:
    assert processor.is_photo(header + b"\x00" * 32) is photo


def test_los_pdf_siguen_el_camino_de_siempre(vision, monkeypatch) -> None:
    def no_debe_llamarse(_image):
        raise AssertionError("un PDF no debe pasar por el lector de fotos")

    monkeypatch.setattr(processor, "parse_ticket_image", no_debe_llamarse)

    response = post_file(PDF, "ticket.pdf", "application/pdf")

    assert response.status_code == 200, response.text
    body = response.json()
    # Sin avisos ni perfil nuevos: la respuesta es la del parser de PDF de siempre
    assert body["warnings"] == []
    assert body["processing_profile"] is None
    esperado = parse_ticket(base64.b64encode(PDF).decode())
    assert body["raw_text"] == esperado.raw_text
    assert vision["requests"] == []


def test_lo_que_no_es_una_foto_tambien_va_al_parser_de_pdf(vision) -> None:
    # Aunque el navegador diga que es una imagen, el contenido manda
    response = post_file(b"PK\x03\x04 esto es un zip", "foto.jpg", "image/jpeg")

    assert response.status_code == 422
    assert response.json() == {"detail": "No se pudo procesar el archivo"}
    assert vision["requests"] == []


# --------------------------------------------------------------------- visión


def test_lee_una_foto_con_el_modelo_de_vision(vision) -> None:
    response = post_file(png_bytes(), "IMG_0001.png", "image/png")

    assert response.status_code == 200, response.text
    body = response.json()
    assert body["processing_profile"] == "image-vision"
    assert body["warnings"] == []
    assert body["numero_factura"] == "0000-000-000001"
    assert body["fecha"] == "01/02/2026"
    assert body["fecha_hora"] == "2026-02-01T18:30:00"
    assert body["total"] == 7.09
    assert body["metodo_pago"] == "TARJETA BANCARIA"

    productos = body["productos"]
    assert [p["nombre"] for p in productos] == ["LECHE ENTERA", "PAN DE MOLDE", "MANZANA", "CAFÉ MOLIDO"]
    assert productos[1]["precio_unitario"] == 1.25  # calculado: total / cantidad
    assert (productos[2]["cantidad"], productos[2]["unidad"]) == (0.82, "kg")
    assert all(p["iva_porcentaje"] in (4.0, 10.0) for p in productos)

    sent = vision["requests"][0]
    assert sent["model"] == "modelo/principal"
    assert sent["response_format"]["type"] == "json_schema"
    image_url = sent["messages"][0]["content"][0]["image_url"]["url"]
    assert image_url.startswith("data:image/jpeg;base64,")


def test_si_el_primer_modelo_falla_prueba_el_siguiente(vision) -> None:
    vision["replies"]["modelo/principal"] = httpx.Response(502, json={"error": "upstream"})

    response = post_file(png_bytes())

    assert response.status_code == 200, response.text
    assert [r["model"] for r in vision["requests"]] == ["modelo/principal", "modelo/respaldo"]


def test_una_respuesta_que_no_es_json_cuenta_como_fallo_del_modelo(vision) -> None:
    vision["replies"]["modelo/principal"] = openrouter_reply("lo siento, no puedo")

    response = post_file(png_bytes())

    assert response.status_code == 200, response.text
    assert len(vision["requests"]) == 2


def test_una_foto_que_no_es_un_ticket_se_rechaza_con_un_mensaje_claro(vision) -> None:
    vision["replies"]["modelo/principal"] = openrouter_reply({**TICKET_JSON, "es_ticket_mercadona": False})

    response = post_file(png_bytes())

    assert response.status_code == 422
    assert response.json() == {"detail": "La imagen no parece un ticket de Mercadona.", "code": "ticket_rejected"}
    assert len(vision["requests"]) == 1  # no se gasta otra llamada en el modelo de respaldo


def test_una_imagen_corrupta_se_rechaza(vision) -> None:
    response = post_file(b"\xff\xd8\xff\xe0" + b"\x00" * 64)

    assert response.status_code == 422
    assert "No se ha podido abrir la imagen" in response.json()["detail"]


# --------------------------------------------------------------------- respaldo


def fake_tesseract_ticket(_image) -> ParsedTicket:
    return ParsedTicket(
        raw_text="MERCADONA",
        numero_factura=None,
        fecha="01/02/2026",
        fecha_hora=None,
        total=None,
        tienda=None,
        ubicacion=None,
        metodo_pago=None,
        numero_operacion=None,
        productos=[ParsedProduct("LECHE", 1, "unidad", 0.95, 0.95)],
    )


def test_sin_modelos_disponibles_usa_tesseract_y_avisa(vision, monkeypatch) -> None:
    for model in ("modelo/principal", "modelo/respaldo"):
        vision["replies"][model] = httpx.Response(429, json={"error": "rate limit"})
    monkeypatch.setattr(processor, "parse_ticket_image", fake_tesseract_ticket)

    response = post_file(png_bytes())

    assert response.status_code == 200, response.text
    body = response.json()
    assert body["processing_profile"] == "image-ocr-local"
    assert body["warnings"][0] == processor.AVISO_LECTOR_LOCAL
    assert "No se ha leído el número de factura." in body["warnings"]


def test_sin_clave_de_openrouter_no_llama_a_ningun_modelo(vision, monkeypatch) -> None:
    monkeypatch.delenv("OPENROUTER_API_KEY")
    monkeypatch.setattr(processor, "parse_ticket_image", fake_tesseract_ticket)

    response = post_file(png_bytes())

    assert response.status_code == 200, response.text
    assert response.json()["processing_profile"] == "image-ocr-local"
    assert vision["requests"] == []


def test_sin_ningun_lector_responde_503(vision, monkeypatch) -> None:
    monkeypatch.delenv("OPENROUTER_API_KEY")

    def no_tesseract(_image):
        raise processor.OcrEngineUnavailableError("sin tesseract")

    monkeypatch.setattr(processor, "parse_ticket_image", no_tesseract)

    assert post_file(png_bytes()).status_code == 503


# --------------------------------------------------------------------- normalización


def test_normaliza_lo_que_devuelve_el_modelo() -> None:
    ticket, warnings = vision_parser.to_parsed_ticket(
        {
            **TICKET_JSON,
            "numero_factura": "0000-00O-000001",  # O en vez de 0: no tiene el formato
            "hora": None,
            "total": "7,09",
            "productos": [
                {"nombre": "  2   leche  entera ", "cantidad": 2, "unidad": "litros", "precio_unitario": None, "precio_total": "1,90"},
                {"nombre": "", "cantidad": 1, "unidad": "unidad", "precio_unitario": 1, "precio_total": 1},
            ],
        }
    )

    assert ticket.numero_factura is None
    assert "número de factura" in warnings[0]
    assert (ticket.fecha, ticket.fecha_hora) == ("01/02/2026", None)
    assert ticket.total == 7.09
    assert len(ticket.productos) == 1
    producto = ticket.productos[0]
    assert (producto.nombre, producto.unidad, producto.precio_unitario, producto.precio_total) == (
        "LECHE ENTERA",
        "unidad",
        0.95,
        1.9,
    )


@pytest.mark.parametrize(
    ("raw", "cantidad", "unidad", "clean"),
    [
        ("1 PAN", 1, "unidad", "PAN"),
        ("2 YOGUR", 2, "unidad", "YOGUR"),
        ("1 BROCOLI", 0.59, "kg", "BROCOLI"),
        ("12 HUEVOS GRANDES-L", 1, "unidad", "12 HUEVOS GRANDES-L"),
        ("1 12 HUEVOS GRANDES-L", 1, "unidad", "12 HUEVOS GRANDES-L"),
        ("12H LASTING LIPS", 1, "unidad", "12H LASTING LIPS"),
        ("7UP", 7, "unidad", "7UP"),
        ("1", 1, "unidad", "1"),
    ],
)
def test_quita_la_cantidad_que_se_cuela_en_el_nombre(raw, cantidad, unidad, clean) -> None:
    assert vision_parser.clean_product_name(raw, cantidad, unidad) == clean


# --------------------------------------------------------------------- avisos y parser


def test_avisa_si_la_suma_de_productos_no_cuadra_con_el_total() -> None:
    ticket = fake_tesseract_ticket(None)
    ticket.numero_factura = "0000-000-000001"
    ticket.total = 9.95

    assert check_ticket(ticket) == [
        "No se ha leído la hora.",
        "La suma de los productos (0,95 €) no coincide con el total (9,95 €).",
    ]


def test_el_lector_local_adapta_la_cabecera_y_el_total_del_ticket_en_papel() -> None:
    texto = "\n".join(
        [
            "Descripción            P. Unit  Imp.(€)",
            "1 LECHE ENTERA                    0,95",
            "2 YOGUR NATURAL         0,50      1,00",
            "TOTAL (e)                         1,95",
            "IVA BASE IMPONIBLE (€) CUOTA (€)",
            "TOTAL          1,77      0,18",
        ]
    )

    # Sin adaptar, el parser de PDF no encuentra la tabla (espera "Importe")
    assert extract_products(texto) == []
    productos = extract_products(adapt_paper_ticket_text(texto))

    assert [(p.nombre, p.cantidad, p.precio_total) for p in productos] == [
        ("LECHE ENTERA", 1.0, 0.95),
        ("YOGUR NATURAL", 2.0, 1.0),
    ]
    # El total es el de la compra, no el de la tabla de IVA
    assert extract_paper_total(texto) == 1.95


@pytest.mark.parametrize(
    ("raw", "metodo"),
    [("EFECTIVO", "EFECTIVO"), ("ENTREGA EFECTIVO", "EFECTIVO"), ("TARJ. BANCARIA", "TARJETA BANCARIA"),
     ("VISA", "TARJETA BANCARIA"), ("tarjeta bancaria", "TARJETA BANCARIA"), ("CHEQUE", None), (None, None)],
)
def test_normaliza_el_metodo_de_pago(raw, metodo) -> None:
    assert vision_parser.normalize_payment(raw) == metodo
