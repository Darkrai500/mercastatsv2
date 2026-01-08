"""
Servicio de parsing de tickets de Mercadona.

Soporta PDFs nativos, PDFs escaneados (fallback a OCR) e imagenes (JPG/PNG/WEBP/HEIC).
"""

import base64
import io
import re
from dataclasses import dataclass, field
from datetime import datetime
from typing import List, Optional, Tuple

import cv2
import numpy as np
import pdfplumber
import pytesseract
from PIL import Image, ImageOps
from loguru import logger

from ..constants import (
    MAX_RAW_TEXT_PREVIEW,
    PATTERN_DIRECCION,
    PATTERN_FECHA,
    PATTERN_FECHA_HORA,
    PATTERN_METODO_PAGO,
    PATTERN_NUMERO_FACTURA,
    PATTERN_NUMERO_OPERACION,
    PATTERN_PRODUCTO_PESADO_DETALLE,
    PATTERN_TOTAL,
    PATTERN_TOTAL_ALT,
    PATTERN_IVA,
)

try:
    import pillow_heif

    pillow_heif.register_heif_opener()
except Exception:  # pragma: no cover - defensivo si no esta instalado
    pillow_heif = None  # type: ignore
    logger.warning("pillow-heif no disponible; los HEIC/HEIF pueden fallar al abrirse")


# ============================================================================
# DATA CLASSES
# ============================================================================


@dataclass
class ParsedProduct:
    """Producto detectado dentro del ticket."""

    nombre: str
    cantidad: float
    unidad: str
    precio_unitario: float
    precio_total: float
    descuento: float = 0.0
    iva_porcentaje: float = 0.0
    iva_importe: float = 0.0


@dataclass
class IvaBreakdown:
    """Desglose de IVA encontrado al final del ticket."""

    porcentaje: float
    base_imponible: float
    cuota: float


@dataclass
class ParsedTicket:
    """Resultado completo del parsing de un ticket."""

    raw_text: str
    numero_factura: Optional[str]
    fecha: Optional[str]
    fecha_hora: Optional[datetime]
    total: Optional[float]
    tienda: Optional[str]
    ubicacion: Optional[str]
    metodo_pago: Optional[str]
    numero_operacion: Optional[str]
    productos: List[ParsedProduct] = field(default_factory=list)
    iva_desglose: List[IvaBreakdown] = field(default_factory=list)
    processing_profile: Optional[str] = None
    warnings: List[str] = field(default_factory=list)


@dataclass
class TextExtractionResult:
    """Resultado de la fase de extraccion de texto (PDF o imagen)."""

    text: str
    processing_profile: str
    warnings: List[str] = field(default_factory=list)


# ============================================================================
# EXCEPCIONES PERSONALIZADAS
# ============================================================================


class PDFParsingError(Exception):
    """Error generico cuando no se puede interpretar el ticket."""


class UnsupportedFormatError(PDFParsingError):
    """Formato de archivo no soportado (ni PDF ni imagen reconocida)."""


class TicketNotDetectedError(PDFParsingError):
    """No se encontro texto suficiente para considerar que hay un ticket."""


# ============================================================================
# FUNCIONES AUXILIARES
# ============================================================================


PRODUCTO_LINE_REGEX = re.compile(
    r"^(?P<cantidad>\d+)\s+(?P<descripcion>.+?)\s+(?P<precio1>\d+,[0-9]{2})(?:\s+(?P<precio2>\d+,[0-9]{2}))?$"
)
PRODUCTO_PESADO_HEADER_REGEX = re.compile(r"^(?P<cantidad>\d+)\s+(?P<descripcion>.+)$")

MIN_TEXT_CHARS = 30
MAX_IMAGE_SIDE = 2000
OCR_CONFIG = "--oem 3 --psm 6"
OCR_LANG = "spa+eng"


def parse_decimal(value: str) -> float:
    """Convierte numeros con coma decimal espanola a float."""
    normalizado = value.replace(".", "").replace(",", ".")
    return float(normalizado)


def clean_text(value: str) -> str:
    """Normaliza espacios redundantes."""
    return re.sub(r"\s+", " ", value).strip()


def detect_source_type(data: bytes) -> str:
    """Detecta si el contenido es PDF o imagen por magic bytes."""
    if data.startswith(b"%PDF"):
        return "pdf"
    if data[:2] == b"\xff\xd8":
        return "image"  # JPEG
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return "image"
    if data[:4] == b"RIFF" and data[8:12] == b"WEBP":
        return "image"
    if data[4:12] in {b"ftypheic", b"ftypheif", b"ftypmif1", b"ftypmsf1"}:
        return "image"
    return "unknown"


def resolve_source_type(data: bytes, mime_type: Optional[str]) -> str:
    """
    Determina el tipo de origen combinando mime_type (si viene del cliente) y magic bytes.

    Priorizamos el mime_type para evitar falsos positivos (ej: imagen tratada como PDF),
    pero dejamos trazas si no coincide con lo que reportan los magic bytes.
    """
    explicit: Optional[str] = None
    if mime_type:
        lowered = mime_type.lower()
        if lowered.startswith("image/"):
            explicit = "image"
        elif lowered == "application/pdf":
            explicit = "pdf"

    detected = detect_source_type(data)

    if explicit and detected not in {"unknown", explicit}:
        logger.warning(
            f"mime_type '{mime_type}' no coincide con magic bytes '{detected}'; se prioriza mime_type"
        )

    return explicit or detected


def ensure_text_threshold(text: str) -> None:
    if len(text.strip()) < MIN_TEXT_CHARS:
        raise TicketNotDetectedError(
            "No se detecto texto suficiente para interpretar el ticket. "
            "Sube un PDF legible o una foto mas nitida."
        )


# ============================================================================
# EXTRACCION DE TEXTO
# ============================================================================


def extract_text_from_pdf(pdf_bytes: bytes) -> TextExtractionResult:
    """Extrae texto de un PDF usando pdfplumber; si no hay texto levanta error."""
    try:
        pdf_buffer = io.BytesIO(pdf_bytes)
        pages_text: List[str] = []

        with pdfplumber.open(pdf_buffer) as pdf:
            logger.info(f"PDF abierto correctamente. Paginas: {len(pdf.pages)}")

            for page_num, page in enumerate(pdf.pages, start=1):
                text = page.extract_text()
                if text:
                    pages_text.append(text)
                    logger.debug(f"Pagina {page_num}: {len(text)} caracteres extraidos")
                else:
                    logger.warning(
                        f"Pagina {page_num}: sin texto interpretable (posible imagen escaneada)"
                    )

        if not pages_text:
            raise PDFParsingError("No se pudo extraer texto del PDF. Puede ser una imagen escaneada.")

        full_text = "\n\n".join(pages_text)
        logger.info(f"Texto extraido exitosamente. Total: {len(full_text)} caracteres")
        ensure_text_threshold(full_text)
        return TextExtractionResult(text=full_text, processing_profile="pdf-text", warnings=[])

    except pdfplumber.pdfminer.pdfparser.PDFSyntaxError as exc:
        logger.error(f"PDF corrupto o invalido: {exc}")
        raise PDFParsingError(f"PDF corrupto: {exc}") from exc
    except TicketNotDetectedError:
        raise
    except Exception as exc:
        logger.error(f"Error inesperado al extraer texto del PDF: {exc}")
        raise PDFParsingError(f"Error al procesar PDF: {exc}") from exc


def preprocess_image_for_ocr(image: Image.Image) -> Tuple[np.ndarray, float]:
    """Normaliza la imagen (RGB->gris, resize, binariza, deskew) para OCR."""
    img = ImageOps.exif_transpose(image).convert("RGB")
    np_img = np.array(img)
    gray = cv2.cvtColor(np_img, cv2.COLOR_RGB2GRAY)

    h, w = gray.shape[:2]
    max_side = max(h, w)
    if max_side > MAX_IMAGE_SIDE:
        scale = MAX_IMAGE_SIDE / float(max_side)
        gray = cv2.resize(gray, (int(w * scale), int(h * scale)), interpolation=cv2.INTER_AREA)
        logger.debug(f"Imagen redimensionada para OCR: {gray.shape[1]}x{gray.shape[0]}")

    denoised = cv2.medianBlur(gray, 3)
    thresh = cv2.adaptiveThreshold(
        denoised, 255, cv2.ADAPTIVE_THRESH_GAUSSIAN_C, cv2.THRESH_BINARY, 31, 15
    )

    deskewed, angle = deskew_image(thresh)
    return deskewed, angle


def deskew_image(gray: np.ndarray) -> Tuple[np.ndarray, float]:
    """Corrige inclinacion basica basada en contornos binarizados."""
    coords = np.column_stack(np.where(gray > 0))
    if coords.size == 0:
        return gray, 0.0

    angle = cv2.minAreaRect(coords)[-1]
    if angle < -45:
        angle = -(90 + angle)
    else:
        angle = -angle

    (h, w) = gray.shape[:2]
    center = (w // 2, h // 2)
    M = cv2.getRotationMatrix2D(center, angle, 1.0)
    rotated = cv2.warpAffine(gray, M, (w, h), flags=cv2.INTER_CUBIC, borderMode=cv2.BORDER_REPLICATE)
    return rotated, angle


def run_ocr(image_np: np.ndarray) -> str:
    """Ejecuta Tesseract y devuelve el texto normalizado."""
    try:
        text = pytesseract.image_to_string(image_np, lang=OCR_LANG, config=OCR_CONFIG)
    except pytesseract.TesseractNotFoundError as exc:
        raise PDFParsingError(
            "Tesseract no esta instalado o no es accesible en PATH. Instala el binario para habilitar OCR."
        ) from exc
    except pytesseract.TesseractError as exc:
        raise PDFParsingError(f"OCR fallo: {exc}") from exc

    return text.strip()


def extract_text_from_image_bytes(image_bytes: bytes, profile: str = "image-ocr") -> TextExtractionResult:
    """Aplica OCR sobre una imagen (JPG/PNG/WEBP/HEIC)."""
    try:
        image = Image.open(io.BytesIO(image_bytes))
    except Exception as exc:
        raise PDFParsingError(f"No se pudo abrir la imagen: {exc}") from exc

    processed, angle = preprocess_image_for_ocr(image)
    text = run_ocr(processed)

    warnings: List[str] = []
    if abs(angle) > 0.5:
        warnings.append(f"Imagen enderezada {angle:.2f} grados para OCR")

    return TextExtractionResult(text=text, processing_profile=profile, warnings=warnings)


def extract_text_from_pdf_as_images(pdf_bytes: bytes) -> TextExtractionResult:
    """Rasteriza el PDF a imagen y aplica OCR pagina por pagina."""
    pdf_buffer = io.BytesIO(pdf_bytes)
    texts: List[str] = []
    warnings: List[str] = ["Texto PDF insuficiente; se aplica OCR sobre imagen"]

    with pdfplumber.open(pdf_buffer) as pdf:
        if not pdf.pages:
            raise PDFParsingError("El PDF no tiene paginas interpretable para OCR")

        for idx, page in enumerate(pdf.pages, start=1):
            try:
                page_image = page.to_image(resolution=300).original
                ocr_result = extract_text_from_image_bytes(
                    image_bytes=image_to_bytes(page_image), profile="pdf-ocr"
                )
                texts.append(ocr_result.text)
                warnings.extend(ocr_result.warnings)
                logger.info(f"OCR de pagina {idx} completado ({len(ocr_result.text)} chars)")
            except Exception as exc:  # pragma: no cover - defensivo
                logger.warning(f"Fallo OCR de la pagina {idx}: {exc}")

    combined = "\n\n".join(texts).strip()
    if not combined:
        raise TicketNotDetectedError(
            "No se pudo extraer texto del PDF escaneado. Reintenta con una foto mas clara."
        )

    return TextExtractionResult(text=combined, processing_profile="pdf-ocr", warnings=warnings)


def image_to_bytes(image: Image.Image) -> bytes:
    buffer = io.BytesIO()
    image.save(buffer, format="PNG")
    return buffer.getvalue()


# ============================================================================
# EXTRACCION DE CAMPOS ESPECIFICOS
# ============================================================================


def extract_numero_factura(text: str) -> Optional[str]:
    match = PATTERN_NUMERO_FACTURA.search(text)
    if match:
        numero = match.group(1)
        logger.info(f"Numero de factura detectado: {numero}")
        return numero

    logger.warning("Numero de factura no detectado")
    return None


def extract_fecha_y_hora(text: str) -> Tuple[Optional[str], Optional[datetime]]:
    match = PATTERN_FECHA_HORA.search(text)
    if match:
        fecha_str, hora_str = match.groups()
        combinado = f"{fecha_str} {hora_str}"
        try:
            fecha_dt = datetime.strptime(combinado, "%d/%m/%Y %H:%M")
            logger.info(f"Fecha y hora detectadas: {fecha_dt.isoformat(timespec='minutes')}")
            return fecha_str, fecha_dt
        except ValueError as exc:
            logger.error(f"Error al parsear fecha y hora '{combinado}': {exc}")

    match = PATTERN_FECHA.search(text)
    if match:
        fecha_str = match.group(1)
        logger.info(f"Fecha detectada (sin hora): {fecha_str}")
        return fecha_str, None

    logger.warning("Fecha no detectada")
    return None, None


def extract_total(text: str) -> Optional[float]:
    match = PATTERN_TOTAL.search(text) or PATTERN_TOTAL_ALT.search(text)
    if match:
        total_str = match.group(1)
        try:
            total_float = round(parse_decimal(total_str), 2)
            logger.info(f"Total detectado: {total_float:.2f} euros")
            return total_float
        except ValueError as exc:
            logger.error(f"Error al convertir total '{total_str}': {exc}")
            return None

    logger.warning("Total no detectado")
    return None


def extract_store_details(text: str) -> Tuple[Optional[str], Optional[str]]:
    store_name: Optional[str] = None
    for line in text.splitlines()[:10]:
        if "MERCADONA" in line.upper():
            store_name = clean_text(line)
            if " A-" in store_name:
                store_name = store_name.split(" A-")[0].strip()
            break

    ubicacion: Optional[str] = None
    match = PATTERN_DIRECCION.search(text)
    if match:
        street = clean_text(match.group(1))
        city = clean_text(match.group(2))
        ubicacion = f"C/ {street}, {city}"

    if store_name:
        logger.info(f"Tienda detectada: {store_name}")
    if ubicacion:
        logger.info(f"Ubicacion detectada: {ubicacion}")

    return store_name, ubicacion


def extract_metodo_pago(text: str) -> Optional[str]:
    match = PATTERN_METODO_PAGO.search(text)
    if match:
        raw = clean_text(match.group(1))
        if raw.upper() in {"MASTERCARD", "VISA", "AMEX"}:
            metodo = raw.upper()
        elif raw.upper().startswith("TARJ"):
            metodo = "Tarjeta bancaria"
        else:
            metodo = raw.title()
        logger.info(f"Metodo de pago detectado: {metodo}")
        return metodo
    return None


def extract_numero_operacion(text: str) -> Optional[str]:
    match = PATTERN_NUMERO_OPERACION.search(text)
    if match:
        numero = match.group(1)
        logger.info(f"Numero de operacion detectado: {numero}")
        return numero

    fallback = re.search(r"N\.C:\s*(\d+)", text, re.IGNORECASE)
    if fallback:
        numero = fallback.group(1)
        logger.info(f"Numero de operacion detectado (N.C): {numero}")
        return numero

    return None


def extract_iva_breakdown(text: str) -> List[IvaBreakdown]:
    desglose: List[IvaBreakdown] = []
    for match in PATTERN_IVA.finditer(text):
        porcentaje = float(match.group(1))
        base = round(parse_decimal(match.group(2)), 2)
        cuota = round(parse_decimal(match.group(3)), 2)
        desglose.append(IvaBreakdown(porcentaje=porcentaje, base_imponible=base, cuota=cuota))
    if desglose:
        logger.info(f"Detectado desglose de IVA con {len(desglose)} tasas")
    return desglose


def extract_products(text: str) -> List[ParsedProduct]:
    productos: List[ParsedProduct] = []
    lines = [line.strip() for line in text.splitlines()]
    in_section = False
    index = 0

    while index < len(lines):
        line = lines[index]
        if not line:
            index += 1
            continue

        normalized = line.lower()
        normalized_clean = normalized.replace("?", "e")
        if "descrip" in normalized_clean and "importe" in normalized_clean:
            in_section = True
            index += 1
            continue

        if not in_section:
            index += 1
            continue

        upper_line = line.upper()
        if upper_line.startswith("TOTAL") or upper_line.startswith("IVA") or upper_line.startswith("TARJ"):
            break

        match_simple = PRODUCTO_LINE_REGEX.match(line)
        if match_simple:
            cantidad = float(match_simple.group("cantidad"))
            descripcion = clean_text(match_simple.group("descripcion"))
            precio_1 = round(parse_decimal(match_simple.group("precio1")), 2)
            precio_2 = match_simple.group("precio2")

            if precio_2:
                precio_total = round(parse_decimal(precio_2), 2)
                precio_unitario = precio_1
            else:
                precio_total = round(precio_1 * cantidad, 2)
                precio_unitario = round(precio_total / cantidad, 2) if cantidad else precio_1

            productos.append(
                ParsedProduct(
                    nombre=descripcion,
                    cantidad=round(cantidad, 3),
                    unidad="unidad",
                    precio_unitario=precio_unitario,
                    precio_total=precio_total,
                )
            )
            index += 1
            continue

        next_line = lines[index + 1].strip() if index + 1 < len(lines) else ""
        match_pesado = (
            PRODUCTO_PESADO_HEADER_REGEX.match(line)
            if next_line
            else None
        )
        detalle_pesado = (
            PATTERN_PRODUCTO_PESADO_DETALLE.match(next_line)
            if next_line
            else None
        )

        if match_pesado and detalle_pesado:
            descripcion = clean_text(match_pesado.group("descripcion"))
            peso = parse_decimal(detalle_pesado.group("peso"))
            unidad = detalle_pesado.group("unidad").lower()
            precio_unitario = round(parse_decimal(detalle_pesado.group("precio_unitario")), 2)
            importe = round(parse_decimal(detalle_pesado.group("importe")), 2)

            cantidad_real = peso
            unidad_normalizada = unidad
            if unidad == "g":
                cantidad_real = round(peso / 1000, 3)
                unidad_normalizada = "kg"
            elif unidad == "ml":
                cantidad_real = round(peso / 1000, 3)
                unidad_normalizada = "l"
            elif unidad == "kg":
                cantidad_real = round(peso, 3)

            productos.append(
                ParsedProduct(
                    nombre=descripcion,
                    cantidad=round(cantidad_real, 3),
                    unidad=unidad_normalizada,
                    precio_unitario=precio_unitario,
                    precio_total=importe,
                )
            )
            index += 2
            continue

        logger.debug(f"No se pudo interpretar la linea de producto: '{line}'")
        index += 1

    logger.info(f"Productos detectados: {len(productos)}")
    return productos


def assign_iva_to_products(productos: List[ParsedProduct], iva_desglose: List[IvaBreakdown]) -> None:
    if not productos or not iva_desglose:
        return

    buckets = [
        {"porcentaje": item.porcentaje, "base": item.base_imponible, "cuota": item.cuota}
        for item in iva_desglose
    ]
    buckets.sort(key=lambda bucket: bucket["porcentaje"], reverse=True)

    for producto in sorted(productos, key=lambda prod: prod.precio_total, reverse=True):
        asignado = False
        for bucket in buckets:
            tasa = bucket["porcentaje"]
            if tasa == 0:
                base_estimada = round(producto.precio_total, 2)
                cuota_estimada = 0.0
            else:
                divisor = 1 + tasa / 100
                base_estimada = round(producto.precio_total / divisor, 2)
                cuota_estimada = round(producto.precio_total - base_estimada, 2)

            tolerancia = 0.05
            if (
                base_estimada <= bucket["base"] + tolerancia
                and cuota_estimada <= bucket["cuota"] + tolerancia
            ):
                producto.iva_porcentaje = tasa
                producto.iva_importe = round(cuota_estimada, 2)
                bucket["base"] = max(0.0, round(bucket["base"] - base_estimada, 2))
                bucket["cuota"] = max(0.0, round(bucket["cuota"] - cuota_estimada, 2))
                asignado = True
                break

        if not asignado and buckets:
            fallback = buckets[0]
            tasa = fallback["porcentaje"]
            if tasa == 0:
                producto.iva_porcentaje = 0.0
                producto.iva_importe = 0.0
            else:
                divisor = 1 + tasa / 100
                base_estimada = round(producto.precio_total / divisor, 2)
                cuota_estimada = round(producto.precio_total - base_estimada, 2)
                producto.iva_porcentaje = tasa
                producto.iva_importe = round(cuota_estimada, 2)


# ============================================================================
# FUNCION PRINCIPAL
# ============================================================================


def parse_ticket(file_b64: str, mime_type: Optional[str] = None) -> ParsedTicket:
    """Orquesta la extraccion completa de un ticket PDF o imagen."""
    logger.info("Iniciando parsing de ticket...")

    try:
        file_bytes = base64.b64decode(file_b64)
        logger.info(f"Archivo decodificado. Tamano: {len(file_bytes)} bytes")

        source_type = resolve_source_type(file_bytes, mime_type)
        if source_type == "unknown":
            raise UnsupportedFormatError(
                "Formato no soportado. Solo se aceptan PDF o imagenes (jpg, png, webp, heic)."
            )

        if source_type == "pdf":
            try:
                text_result = extract_text_from_pdf(file_bytes)
            except PDFParsingError as exc:
                logger.warning(f"Lectura directa de PDF fallida: {exc}")
                text_result = extract_text_from_pdf_as_images(file_bytes)
            except TicketNotDetectedError:
                text_result = extract_text_from_pdf_as_images(file_bytes)
        elif source_type == "image":
            text_result = extract_text_from_image_bytes(file_bytes)
        else:
            raise UnsupportedFormatError(
                "Formato no soportado. Solo se aceptan PDF o imagenes (jpg, png, webp, heic)."
            )

        ensure_text_threshold(text_result.text)
        preview = text_result.text[:MAX_RAW_TEXT_PREVIEW].replace("\n", " ")
        logger.debug(f"Preview del texto: {preview}...")

        numero_factura = extract_numero_factura(text_result.text)
        fecha, fecha_hora = extract_fecha_y_hora(text_result.text)
        total = extract_total(text_result.text)
        tienda, ubicacion = extract_store_details(text_result.text)
        metodo_pago = extract_metodo_pago(text_result.text)
        numero_operacion = extract_numero_operacion(text_result.text)
        iva_desglose = extract_iva_breakdown(text_result.text)
        productos = extract_products(text_result.text)
        assign_iva_to_products(productos, iva_desglose)

        logger.info(
            "Parsing completado: factura=%s fecha=%s fecha_hora=%s total=%s productos=%s profile=%s",
            numero_factura,
            fecha,
            fecha_hora.isoformat(timespec="minutes") if fecha_hora else None,
            total,
            len(productos),
            text_result.processing_profile,
        )

        return ParsedTicket(
            raw_text=text_result.text,
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
            processing_profile=text_result.processing_profile,
            warnings=text_result.warnings,
        )

    except base64.binascii.Error as exc:
        logger.error(f"Error al decodificar base64: {exc}")
        raise PDFParsingError(f"Base64 invalido: {exc}") from exc
    except UnsupportedFormatError:
        raise
    except TicketNotDetectedError:
        raise
    except Exception as exc:
        logger.error(f"Error inesperado durante el parsing: {exc}")
        raise


__all__ = [
    "parse_ticket",
    "PDFParsingError",
    "TicketNotDetectedError",
    "UnsupportedFormatError",
    "ParsedTicket",
    "ParsedProduct",
    "IvaBreakdown",
]
