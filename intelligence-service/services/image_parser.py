"""
Servicio de parsing de tickets en formato imagen (JPG, PNG).

Usa pytesseract para OCR y Pillow para preprocesamiento.
Reutiliza la logica de extraccion de pdf_parser.
"""

import base64
import io
from typing import List

import pytesseract
from PIL import Image, ImageEnhance, ImageOps
from loguru import logger

from services.pdf_parser import (
    ImageParsingError,
    ParsedTicket,
    extract_numero_factura,
    extract_fecha_y_hora,
    extract_total,
    extract_store_details,
    extract_metodo_pago,
    extract_numero_operacion,
    extract_iva_breakdown,
    extract_products,
    assign_iva_to_products,
)

# Si Tesseract no esta en el PATH, descomentar y ajustar ruta:
# pytesseract.pytesseract.tesseract_cmd = r'C:\Program Files\Tesseract-OCR\tesseract.exe'


def preprocess_image(image: Image.Image) -> Image.Image:
    """
    Aplica mejoras a la imagen para facilitar el OCR.
    - Escala de grises
    - Aumento de contraste
    - Umbralizacion (opcional, a veces ayuda, a veces no)
    """
    # 1. Escala de grises
    img_gray = ImageOps.grayscale(image)

    # 2. Aumentar contraste
    enhancer = ImageEnhance.Contrast(img_gray)
    img_contrast = enhancer.enhance(2.0)  # Aumentar contraste al doble

    # 3. Sharpening (opcional)
    # enhancer_sharp = ImageEnhance.Sharpness(img_contrast)
    # img_sharp = enhancer_sharp.enhance(1.5)

    return img_contrast


def extract_text_from_image(image_bytes: bytes) -> str:
    """Extrae texto de una imagen usando Tesseract."""
    try:
        image = Image.open(io.BytesIO(image_bytes))
        logger.info(f"Imagen cargada. Formato: {image.format}, Tamano: {image.size}")

        # Preprocesamiento
        processed_image = preprocess_image(image)
        
        # OCR
        # --psm 4: Assume a single column of text of variable sizes.
        # -l spa: Idioma español
        text = pytesseract.image_to_string(processed_image, lang="spa", config="--psm 4")
        
        if not text.strip():
            # Intentar sin preprocesamiento si falla
            logger.warning("OCR con preprocesamiento vacio, reintentando con imagen original")
            text = pytesseract.image_to_string(image, lang="spa", config="--psm 4")

        if not text.strip():
             raise ImageParsingError("No se pudo extraer texto de la imagen (OCR vacio).")

        logger.info(f"Texto extraido de imagen. Longitud: {len(text)}")
        return text

    except Exception as exc:
        logger.error(f"Error en OCR de imagen: {exc}")
        raise ImageParsingError(f"Fallo al procesar imagen: {exc}") from exc


def parse_ticket_image(image_b64: str) -> ParsedTicket:
    """Orquesta la extraccion completa de un ticket desde imagen."""
    logger.info("Iniciando parsing de ticket (Imagen)...")

    try:
        image_bytes = base64.b64decode(image_b64)
        
        raw_text = extract_text_from_image(image_bytes)
        
        # Reutilizar logica de extraccion de pdf_parser
        numero_factura = extract_numero_factura(raw_text)
        fecha, fecha_hora = extract_fecha_y_hora(raw_text)
        total = extract_total(raw_text)
        tienda, ubicacion = extract_store_details(raw_text)
        metodo_pago = extract_metodo_pago(raw_text)
        numero_operacion = extract_numero_operacion(raw_text)
        iva_desglose = extract_iva_breakdown(raw_text)
        productos = extract_products(raw_text)
        assign_iva_to_products(productos, iva_desglose)

        logger.info(
            "Parsing imagen completado: factura=%s fecha=%s total=%s productos=%s",
            numero_factura,
            fecha,
            total,
            len(productos),
        )

        return ParsedTicket(
            raw_text=raw_text,
            numero_factura=numero_factura,
            fecha=fecha,
            fecha_hora=fecha_hora,
            total=total,
            tienda=tienda,
            ubicacion=ubicacion,
            metodo_pago=metodo_pago,
            numero_operacion=numero_operacion,
            productos=productos,
            iva_desglose=iva_desglose,
        )

    except base64.binascii.Error as exc:
        logger.error(f"Error al decodificar base64: {exc}")
        raise ImageParsingError(f"Base64 invalido: {exc}") from exc
    except Exception as exc:
        logger.error(f"Error inesperado durante el parsing de imagen: {exc}")
        raise
