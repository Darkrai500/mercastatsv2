"""
Helpers to expose the parsing logic without coupling to FastAPI.

This module is reused by the FastAPI app as a thin wrapper around the parser.
"""

from __future__ import annotations

import json
from dataclasses import asdict

from models import ProcessTicketRequest, ProcessTicketResponse
from services.pdf_parser import PDFParsingError, parse_ticket as parse_ticket_pdf
from services.image_parser import ImageParsingError, parse_ticket_image


def process_ticket_response(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
    mime_type: str | None = None,
) -> ProcessTicketResponse:
    """
    Ejecuta el parsing y devuelve un modelo Pydantic listo para serializar.
    """
    request = ProcessTicketRequest(
        ticket_id=ticket_id,
        file_name=file_name,
        file_content_b64=pdf_b64,
        mime_type=mime_type,
    )

    # Determinar estrategia de parsing
    is_image = False
    if request.mime_type and request.mime_type.startswith("image/"):
        is_image = True
    elif request.file_name.lower().endswith((".jpg", ".jpeg", ".png")):
        is_image = True

    parsed_ticket = None
    
    if is_image:
        parsed_ticket = parse_ticket_image(request.file_content_b64)
    else:
        try:
            parsed_ticket = parse_ticket_pdf(request.file_content_b64)
        except PDFParsingError:
            # Fallback: si falla como PDF, intentar como imagen (por si acaso es un PDF con imagen incrustada o mal etiquetado)
            # Nota: Esto requeriria convertir PDF a imagen primero, lo cual es complejo sin poppler.
            # Por ahora, si falla PDF y no es imagen explicita, fallamos.
            # Pero si el usuario subio una imagen con extension .pdf (raro) o sin mime type...
            # Vamos a asumir que si falla PDFParsingError es fatal para PDFs reales.
            raise

    return ProcessTicketResponse(
        ticket_id=request.ticket_id,
        raw_text=parsed_ticket.raw_text,
        numero_factura=parsed_ticket.numero_factura,
        fecha=parsed_ticket.fecha,
        fecha_hora=parsed_ticket.fecha_hora,
        total=parsed_ticket.total,
        tienda=parsed_ticket.tienda,
        ubicacion=parsed_ticket.ubicacion,
        metodo_pago=parsed_ticket.metodo_pago,
        numero_operacion=parsed_ticket.numero_operacion,
        productos=[asdict(prod) for prod in parsed_ticket.productos],
        iva_desglose=[asdict(item) for item in parsed_ticket.iva_desglose],
    )


def process_ticket_payload(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
    mime_type: str | None = None,
) -> dict:
    """
    Devuelve la respuesta como diccionario preparado para JSON.
    """
    response = process_ticket_response(ticket_id, file_name, pdf_b64, mime_type)
    return response.model_dump(mode="json")


def process_ticket_json(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
    mime_type: str | None = None,
) -> str:
    """
    Devuelve la respuesta serializada en JSON.
    """
    payload = process_ticket_payload(ticket_id, file_name, pdf_b64, mime_type)
    return json.dumps(payload, ensure_ascii=False)


__all__ = [
    "process_ticket_response",
    "process_ticket_payload",
    "process_ticket_json",
    "PDFParsingError",
    "ImageParsingError",
]
