"""
Helpers para exponer la lógica de parsing sin depender de FastAPI.

Este módulo se reutiliza desde el backend de Rust (vía PyO3) y desde
la propia aplicación FastAPI para evitar duplicar lógica de ensamblado
de la respuesta.
"""

from __future__ import annotations

import json
from dataclasses import asdict

from .models import ProcessTicketRequest, ProcessTicketResponse
from .services import PDFParsingError, parse_ticket


def process_ticket_response(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
) -> ProcessTicketResponse:
    """
    Ejecuta el parsing y devuelve un modelo Pydantic listo para serializar.
    """
    request = ProcessTicketRequest(
        ticket_id=ticket_id,
        file_name=file_name,
        pdf_b64=pdf_b64,
    )

    parsed_ticket = parse_ticket(request.pdf_b64)

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
) -> dict:
    """
    Devuelve la respuesta como diccionario preparado para JSON.
    """
    response = process_ticket_response(ticket_id, file_name, pdf_b64)
    return response.model_dump(mode="json")


def process_ticket_json(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
) -> str:
    """
    Devuelve la respuesta serializada en JSON.
    """
    payload = process_ticket_payload(ticket_id, file_name, pdf_b64)
    return json.dumps(payload, ensure_ascii=False)


__all__ = [
    "process_ticket_response",
    "process_ticket_payload",
    "process_ticket_json",
    "PDFParsingError",
]
