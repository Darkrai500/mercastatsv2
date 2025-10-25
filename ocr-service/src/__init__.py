"""Worker OCR para procesamiento de tickets de Mercadona."""

from .processor import (
    PDFParsingError,
    process_ticket_json,
    process_ticket_payload,
    process_ticket_response,
)

__all__ = [
    "process_ticket_response",
    "process_ticket_payload",
    "process_ticket_json",
    "PDFParsingError",
]

__version__ = "1.0.0"
