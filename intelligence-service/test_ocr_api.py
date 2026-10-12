"""Contrato HTTP de /ocr/process con el backend."""

import base64

from fastapi.testclient import TestClient

from main import app

# PDF mínimo con una sola línea de texto
PDF = (
    b"%PDF-1.1\n1 0 obj<</Type/Catalog/Pages 2 0 R>>endobj\n"
    b"2 0 obj<</Type/Pages/Kids[3 0 R]/Count 1>>endobj\n"
    b"3 0 obj<</Type/Page/Parent 2 0 R/MediaBox[0 0 200 200]/Contents 4 0 R"
    b"/Resources<</Font<</F1 5 0 R>>>>>>endobj\n"
    b"4 0 obj<</Length 44>>stream\nBT /F1 12 Tf 20 100 Td (MERCADONA prueba) Tj ET\nendstream endobj\n"
    b"5 0 obj<</Type/Font/Subtype/Type1/BaseFont/Helvetica>>endobj\n"
    b"trailer<</Root 1 0 R/Size 6>>\n%%EOF\n"
)


def test_ocr_process_responde_200_con_avisos(monkeypatch) -> None:
    # Regresión: la respuesta no declaraba `warnings` y el log del endpoint daba un 500
    monkeypatch.delenv("INTELLIGENCE_API_KEY", raising=False)
    client = TestClient(app)

    response = client.post(
        "/ocr/process",
        json={
            "ticket_id": "t1",
            "file_name": "t.pdf",
            "file_content_b64": base64.b64encode(PDF).decode(),
            "mime_type": "application/pdf",
        },
    )

    assert response.status_code == 200, response.text
    assert response.json()["warnings"] == []
