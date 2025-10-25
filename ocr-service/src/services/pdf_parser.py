"""
Servicio de parsing de tickets de Mercadona.

Usa pdfplumber para extraer el texto del PDF y aplica expresiones
regulares y heuristicas para obtener los datos necesarios
segun el schema de Mercastats.
"""

import base64
import io
import re
from dataclasses import dataclass, field
from datetime import datetime
from typing import List, Optional

import pdfplumber
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


# ============================================================================
# EXCEPCIÓN PERSONALIZADA
# ============================================================================


class PDFParsingError(Exception):
    """Error especifico cuando no se puede interpretar el PDF."""


# ============================================================================
# FUNCIONES AUXILIARES
# ============================================================================


PRODUCTO_LINE_REGEX = re.compile(
    r"^(?P<cantidad>\d+)\s+(?P<descripcion>.+?)\s+(?P<precio1>\d+,[0-9]{2})(?:\s+(?P<precio2>\d+,[0-9]{2}))?$"
)
PRODUCTO_PESADO_HEADER_REGEX = re.compile(r"^(?P<cantidad>\d+)\s+(?P<descripcion>.+)$")


def parse_decimal(value: str) -> float:
    """Convierte numeros con coma decimal española a float."""
    normalizado = value.replace(".", "").replace(",", ".")
    return float(normalizado)


def clean_text(value: str) -> str:
    """Normaliza espacios redundantes."""
    return re.sub(r"\s+", " ", value).strip()


# ============================================================================
# EXTRACCION DE TEXTO
# ============================================================================


def extract_text_from_pdf(pdf_bytes: bytes) -> str:
    """Extrae todo el texto de un PDF usando pdfplumber."""
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
        return full_text

    except pdfplumber.pdfminer.pdfparser.PDFSyntaxError as exc:
        logger.error(f"PDF corrupto o invalido: {exc}")
        raise PDFParsingError(f"PDF corrupto: {exc}") from exc
    except Exception as exc:
        logger.error(f"Error inesperado al extraer texto del PDF: {exc}")
        raise PDFParsingError(f"Error al procesar PDF: {exc}") from exc


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


def extract_fecha_y_hora(text: str) -> (Optional[str], Optional[datetime]):
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


def extract_store_details(text: str) -> (Optional[str], Optional[str]):
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
        normalized_clean = normalized.replace("�", "e")
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


def parse_ticket(pdf_b64: str) -> ParsedTicket:
    """Orquesta la extraccion completa de un ticket."""
    logger.info("Iniciando parsing de ticket...")

    try:
        pdf_bytes = base64.b64decode(pdf_b64)
        logger.info(f"PDF decodificado. Tamano: {len(pdf_bytes)} bytes")

        raw_text = extract_text_from_pdf(pdf_bytes)
        preview = raw_text[:MAX_RAW_TEXT_PREVIEW].replace("\n", " ")
        logger.debug(f"Preview del texto: {preview}...")

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
            "Parsing completado: factura=%s fecha=%s fecha_hora=%s total=%s productos=%s",
            numero_factura,
            fecha,
            fecha_hora.isoformat(timespec="minutes") if fecha_hora else None,
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
        raise PDFParsingError(f"Base64 invalido: {exc}") from exc
    except Exception as exc:
        logger.error(f"Error inesperado durante el parsing: {exc}")
        raise
