"""Errores de la lectura de fotos de tickets (el camino de los PDF no los usa)."""

from services.pdf_parser import ImageParsingError


class TicketRejectedError(ImageParsingError):
    """La foto no contiene un ticket legible. El mensaje se puede mostrar al usuario."""
