"""
Helpers to expose the parsing logic without coupling to FastAPI.

This module is reused by the FastAPI app as a thin wrapper around the parser.
"""

from __future__ import annotations

import json
from dataclasses import asdict
from typing import Optional

from .models import ProcessTicketRequest, ProcessTicketResponse
from .services import (
    PDFParsingError,
    TicketNotDetectedError,
    UnsupportedFormatError,
    parse_ticket,
)


def process_ticket_response(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
    mime_type: Optional[str] = None,
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

    parsed_ticket = parse_ticket(request.file_content_b64, request.mime_type)

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
        processing_profile=parsed_ticket.processing_profile,
        warnings=parsed_ticket.warnings,
    )


def process_ticket_payload(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
    mime_type: Optional[str] = None,
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
    mime_type: Optional[str] = None,
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
    "TicketNotDetectedError",
    "UnsupportedFormatError",
]
