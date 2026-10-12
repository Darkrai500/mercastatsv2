"""
Helpers to expose the parsing logic without coupling to FastAPI.

This module is reused by the FastAPI app as a thin wrapper around the parser.

Las fotos se reconocen por su contenido (bytes mágicos), no por el MIME ni la extensión
que declara el cliente, y se leen con un modelo de visión o, de respaldo, con Tesseract.
Todo lo demás sigue exactamente el camino de siempre del PDF digital (pdfplumber).
"""

from __future__ import annotations

import base64
import binascii
import json
from dataclasses import asdict
from typing import List, Optional, Tuple

from loguru import logger
from PIL import Image

from models import ProcessTicketResponse
from services.image_parser import OcrEngineUnavailableError, parse_ticket_image
from services.pdf_parser import (
    ImageParsingError,
    ParsedTicket,
    PDFParsingError,
    parse_ticket as parse_ticket_pdf,
)
from services.photo_errors import TicketRejectedError
from services.ticket_checks import check_ticket
from services.vision_parser import (
    VisionUnavailableError,
    load_image,
    parse_ticket_photo,
    vision_enabled,
)

# El backend ya limita la subida a 10 MB; esto protege al servicio si se llama directamente.
MAX_PHOTO_BYTES = 15 * 1024 * 1024

HEIF_BRANDS = {b"heic", b"heix", b"hevc", b"hevx", b"heim", b"heis", b"mif1", b"msf1"}

AVISO_LECTOR_LOCAL = (
    "Leído con el lector local (sin IA), que es poco fiable con fotos: "
    "revisa con cuidado los importes y los nombres."
)


class TicketReaderUnavailableError(Exception):
    """No hay ningún lector de fotos operativo (ni modelo de visión ni Tesseract)."""


def is_photo(data: bytes) -> bool:
    """JPEG, PNG, WEBP o HEIC/HEIF según la cabecera del archivo."""
    if data.startswith(b"\xff\xd8\xff") or data.startswith(b"\x89PNG\r\n\x1a\n"):
        return True
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        return True
    return data[4:8] == b"ftyp" and data[8:12] in HEIF_BRANDS


def decode_photo(file_b64: str) -> Optional[bytes]:
    """Bytes de la foto, o ``None`` si el archivo no es una foto (p. ej. un PDF)."""
    try:
        data = base64.b64decode(file_b64)
    except (binascii.Error, ValueError):
        return None
    return data if is_photo(data) else None


def read_photo(image: Image.Image) -> Tuple[ParsedTicket, str, List[str]]:
    """Lee una foto con el modelo de visión y, si no está disponible, con Tesseract."""
    if vision_enabled():
        try:
            result = parse_ticket_photo(image)
            return result.ticket, "image-vision", result.warnings
        except VisionUnavailableError:
            logger.warning("Lector por visión no disponible; se usa Tesseract")

    try:
        return parse_ticket_image(image), "image-ocr-local", [AVISO_LECTOR_LOCAL]
    except OcrEngineUnavailableError as exc:
        raise TicketReaderUnavailableError("No hay ningún lector de fotos disponible") from exc


def process_ticket_response(
    ticket_id: str,
    file_name: str,
    pdf_b64: str,
    mime_type: str | None = None,
) -> ProcessTicketResponse:
    """
    Ejecuta el parsing y devuelve un modelo Pydantic listo para serializar.
    """
    profile: Optional[str] = None
    warnings: List[str] = []

    photo = decode_photo(pdf_b64)
    if photo is not None:
        if len(photo) > MAX_PHOTO_BYTES:
            raise TicketRejectedError("La foto es demasiado grande (máximo 10 MB).")
        parsed_ticket, profile, warnings = read_photo(load_image(photo))
        warnings = warnings + check_ticket(parsed_ticket)
    else:
        parsed_ticket = parse_ticket_pdf(pdf_b64)

    return ProcessTicketResponse(
        ticket_id=ticket_id,
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
        processing_profile=profile,
        warnings=warnings,
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
    "is_photo",
    "PDFParsingError",
    "ImageParsingError",
    "TicketRejectedError",
    "TicketReaderUnavailableError",
]
