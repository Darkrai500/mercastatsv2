"""
Lectura local de fotos de tickets con Tesseract (respaldo del lector por visión).

Se usa cuando no hay modelo de visión configurado o no responde. Con fotos reales acierta
bastante menos (pierde dígitos de importes y líneas enteras), así que su resultado siempre
llega a la revisión con un aviso para que el usuario lo corrija.
"""

import re

import cv2
import numpy as np
import pytesseract
from loguru import logger
from PIL import Image

from services.pdf_parser import (
    ParsedTicket,
    assign_iva_to_products,
    extract_fecha_y_hora,
    extract_iva_breakdown,
    extract_metodo_pago,
    extract_numero_factura,
    extract_numero_operacion,
    extract_products,
    extract_store_details,
    extract_total,
    parse_decimal,
    strip_accents,
)
from services.photo_errors import TicketRejectedError

# Ancho mínimo de trabajo: Tesseract lee mejor con caracteres de ~25-35 px de alto.
MIN_WIDTH = 1400

# TOTAL de la compra con el símbolo € mal leído por el OCR: "TOTAL (e) 12,34", "TOTAL (£) ..."
TOTAL_FOTO_RE = re.compile(r"TOTAL\s*\([^)\d]{0,3}\)\s*([0-9]+,[0-9]{2})", re.IGNORECASE)


class OcrEngineUnavailableError(Exception):
    """El binario de Tesseract (o su idioma español) no está instalado."""


def preprocess_image(image: Image.Image) -> Image.Image:
    """Escala de grises, ampliación y corrección de iluminación (sombras y degradados)."""
    gray = cv2.cvtColor(np.asarray(image.convert("RGB")), cv2.COLOR_RGB2GRAY)
    if gray.shape[1] < MIN_WIDTH:
        factor = MIN_WIDTH / gray.shape[1]
        gray = cv2.resize(gray, None, fx=factor, fy=factor, interpolation=cv2.INTER_CUBIC)
    # Dividir por el fondo estimado deja el papel uniforme sin tocar el trazo del texto
    background = cv2.morphologyEx(gray, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_RECT, (31, 31)))
    background = cv2.GaussianBlur(background, (0, 0), 15)
    return Image.fromarray(cv2.divide(gray, background, scale=255))


def extract_text_from_image(image: Image.Image) -> str:
    """Texto de la foto según Tesseract (columna única de texto, idioma español)."""
    try:
        text = pytesseract.image_to_string(
            preprocess_image(image),
            lang="spa",
            config="--psm 4 -c preserve_interword_spaces=1",
        )
    except (pytesseract.TesseractNotFoundError, pytesseract.TesseractError) as exc:
        logger.error("Tesseract no disponible | tipo={}", type(exc).__name__)
        raise OcrEngineUnavailableError("Tesseract no está disponible") from exc

    if len(text.strip()) < 20:
        raise TicketRejectedError(
            "No se ha podido leer texto en la imagen. Prueba con una foto más nítida y bien iluminada."
        )
    return text


def adapt_paper_ticket_text(text: str) -> str:
    """Adapta el texto de un ticket en papel a lo que espera el parser de PDF.

    En papel la cabecera de la tabla es "Descripción  P. Unit  Imp.(€)" en lugar de
    "... Importe", y sin ella el parser no encuentra la lista de productos.
    """
    lines = []
    for line in text.splitlines():
        normalized = strip_accents(line.lower())
        if "descrip" in normalized and "importe" not in normalized and "imp" in normalized:
            line = f"{line} Importe"
        lines.append(line)
    return "\n".join(lines)


def extract_paper_total(text: str):
    match = TOTAL_FOTO_RE.search(text)
    if match:
        return round(parse_decimal(match.group(1)), 2)
    return extract_total(text)


def parse_ticket_image(image: Image.Image) -> ParsedTicket:
    """Lee una foto de ticket con Tesseract y aplica las mismas reglas que a un PDF."""
    raw_text = extract_text_from_image(image)
    if "MERCADONA" not in raw_text.upper() and "FACTURA" not in raw_text.upper():
        raise TicketRejectedError("La imagen no parece un ticket de Mercadona.")

    fecha, fecha_hora = extract_fecha_y_hora(raw_text)
    tienda, ubicacion = extract_store_details(raw_text)
    iva_desglose = extract_iva_breakdown(raw_text)
    productos = extract_products(adapt_paper_ticket_text(raw_text))
    assign_iva_to_products(productos, iva_desglose)

    logger.info("Ticket leído con Tesseract | productos={}", len(productos))
    return ParsedTicket(
        raw_text=raw_text,
        numero_factura=extract_numero_factura(raw_text),
        fecha=fecha,
        fecha_hora=fecha_hora,
        total=extract_paper_total(raw_text),
        tienda=tienda,
        ubicacion=ubicacion,
        metodo_pago=extract_metodo_pago(raw_text),
        numero_operacion=extract_numero_operacion(raw_text),
        productos=productos,
        iva_desglose=iva_desglose,
    )
