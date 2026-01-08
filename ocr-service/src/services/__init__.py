"""Modulo de servicios para procesamiento de tickets."""

from .pdf_parser import (
    PDFParsingError,
    ParsedTicket,
    TicketNotDetectedError,
    UnsupportedFormatError,
    parse_ticket,
)

__all__ = [
    "parse_ticket",
    "PDFParsingError",
    "TicketNotDetectedError",
    "UnsupportedFormatError",
    "ParsedTicket",
]
