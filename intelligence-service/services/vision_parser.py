"""
Lectura de tickets fotografiados con un modelo de visión a través de OpenRouter.

Las fotos de tickets en papel (perspectiva, pliegues, sombras, tinta térmica gastada)
superan a Tesseract: en las pruebas con fotos reales, Tesseract perdía la mitad de los
productos y dígitos de los importes, mientras que un modelo de visión pequeño leía el
ticket completo por ~0,001 $ por foto. El modelo devuelve JSON con un esquema estricto
y aquí se normaliza al mismo ``ParsedTicket`` que produce el parser de PDF.

Configuración (variables de entorno):
    OPENROUTER_API_KEY   clave de OpenRouter; sin ella este lector queda desactivado
    OCR_VISION_MODELS    modelos a probar en orden, separados por comas
    OCR_VISION_TIMEOUT   segundos máximos en total para leer una foto (todos los modelos)
"""

from __future__ import annotations

import base64
import io
import json
import os
import re
import time
from dataclasses import dataclass
from datetime import datetime
from typing import List, Optional, Tuple

import httpx
from loguru import logger
from PIL import Image, ImageOps, UnidentifiedImageError

from services.pdf_parser import (
    IvaBreakdown,
    ParsedProduct,
    ParsedTicket,
    assign_iva_to_products,
)
from services.photo_errors import TicketRejectedError

try:  # HEIC/HEIF (fotos de iPhone)
    from pillow_heif import register_heif_opener

    register_heif_opener()
except ImportError:  # pragma: no cover - dependencia opcional en desarrollo
    logger.warning("pillow-heif no disponible: las fotos HEIC no se podrán leer")

DEFAULT_MODELS = "anthropic/claude-haiku-5.5,google/gemini-3.1-flash-lite"
DEFAULT_BASE_URL = "https://openrouter.ai/api/v1"
# Lado largo máximo de la imagen enviada: más resolución no mejora la lectura de un
# ticket y encarece la llamada (los tokens de imagen crecen con los píxeles).
MAX_IMAGE_SIDE = 2000
# Un intento con menos margen que esto no llega a completarse (las lecturas tardan 4-11 s)
MIN_ATTEMPT_SECONDS = 4.0

NUMERO_FACTURA_RE = re.compile(r"^\d{4}-\d{3}-\d{6}$")
CANTIDAD_EN_NOMBRE_RE = re.compile(r"^\d+(?:[.,]\d+)?\s+(?=\S)")

TICKET_SCHEMA = {
    "type": "object",
    "additionalProperties": False,
    "required": [
        "es_ticket_mercadona",
        "numero_factura",
        "fecha",
        "hora",
        "numero_operacion",
        "tienda",
        "direccion",
        "metodo_pago",
        "total",
        "productos",
        "iva_desglose",
    ],
    "properties": {
        "es_ticket_mercadona": {"type": "boolean"},
        "numero_factura": {"type": ["string", "null"]},
        "fecha": {"type": ["string", "null"]},
        "hora": {"type": ["string", "null"]},
        "numero_operacion": {"type": ["string", "null"]},
        "tienda": {"type": ["string", "null"]},
        "direccion": {"type": ["string", "null"]},
        "metodo_pago": {"type": ["string", "null"]},
        "total": {"type": ["number", "null"]},
        "productos": {
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": False,
                "required": ["nombre", "cantidad", "unidad", "precio_unitario", "precio_total"],
                "properties": {
                    "nombre": {"type": "string"},
                    "cantidad": {"type": "number"},
                    "unidad": {"type": "string", "enum": ["unidad", "kg", "l"]},
                    "precio_unitario": {"type": ["number", "null"]},
                    "precio_total": {"type": "number"},
                },
            },
        },
        "iva_desglose": {
            "type": "array",
            "items": {
                "type": "object",
                "additionalProperties": False,
                "required": ["porcentaje", "base_imponible", "cuota"],
                "properties": {
                    "porcentaje": {"type": "number"},
                    "base_imponible": {"type": "number"},
                    "cuota": {"type": "number"},
                },
            },
        },
    },
}

PROMPT = """Extrae los datos de esta foto de un ticket de compra de Mercadona.

Reglas:
- Transcribe los nombres de producto exactamente como aparecen impresos (mayúsculas, puntos, abreviaturas y tildes incluidas). No los corrijas ni los completes, y no incluyas la cantidad inicial en el nombre.
- Los importes usan coma decimal en el ticket; devuélvelos como números JSON con punto (2,50 -> 2.5).
- Cada línea de producto empieza por la cantidad. Si hay precio unitario (columna "P. Unit") úsalo; si no, precio_unitario = precio_total / cantidad.
- Los productos a peso ocupan dos líneas: "1 BROCOLI" y debajo "0,590 kg 2,70 €/kg 1,59" -> cantidad 0.59, unidad "kg", precio_unitario 2.70, precio_total 1.59.
- total es el "TOTAL (€)" de la compra, no el total de la tabla de IVA.
- fecha en formato DD/MM/AAAA, hora en HH:MM, numero_factura con el formato NNNN-NNN-NNNNNN.
- metodo_pago: "EFECTIVO" o "TARJETA BANCARIA" según el ticket.
- Si un dato no se lee con seguridad, devuelve null; no lo inventes.
- Si la imagen no es un ticket de Mercadona, es_ticket_mercadona = false.

Responde solo con el JSON."""


class VisionUnavailableError(Exception):
    """Ningún modelo de visión pudo leer la imagen (sin clave, red, cuota, JSON roto...)."""


@dataclass
class VisionResult:
    ticket: ParsedTicket
    model: str
    warnings: List[str]


def vision_models() -> List[str]:
    raw = os.getenv("OCR_VISION_MODELS", DEFAULT_MODELS)
    return [model.strip() for model in raw.split(",") if model.strip()]


def vision_enabled() -> bool:
    return bool(os.getenv("OPENROUTER_API_KEY", "").strip()) and bool(vision_models())


# ============================================================================
# IMAGEN
# ============================================================================


def load_image(image_bytes: bytes) -> Image.Image:
    """Abre la foto respetando la orientación EXIF y la deja en RGB."""
    try:
        image = Image.open(io.BytesIO(image_bytes))
        image.load()
    except (UnidentifiedImageError, OSError, ValueError) as exc:
        raise TicketRejectedError("No se ha podido abrir la imagen. Prueba con una foto JPG o PNG.") from exc
    image = ImageOps.exif_transpose(image)
    if image.mode != "RGB":
        image = image.convert("RGB")
    return image


def to_model_jpeg(image: Image.Image) -> bytes:
    """Reduce la foto a un tamaño razonable y la codifica en JPEG para el modelo."""
    image = image.copy()
    image.thumbnail((MAX_IMAGE_SIDE, MAX_IMAGE_SIDE), Image.LANCZOS)
    buffer = io.BytesIO()
    image.save(buffer, format="JPEG", quality=88)
    return buffer.getvalue()


# ============================================================================
# LLAMADA AL MODELO
# ============================================================================


def _request_body(model: str, jpeg_b64: str) -> dict:
    return {
        "model": model,
        "messages": [
            {
                "role": "user",
                "content": [
                    {"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{jpeg_b64}"}},
                    {"type": "text", "text": PROMPT},
                ],
            }
        ],
        "response_format": {
            "type": "json_schema",
            "json_schema": {"name": "ticket", "strict": True, "schema": TICKET_SCHEMA},
        },
        # Leer un ticket no necesita razonar mucho; esfuerzo bajo = más rápido y barato.
        "reasoning": {"effort": "low"},
        "max_tokens": 8000,
        "usage": {"include": True},
    }


def _parse_content(content: str) -> dict:
    content = re.sub(r"^```(?:json)?\s*|\s*```$", "", (content or "").strip())
    data = json.loads(content)
    # Algún modelo envuelve el objeto en una lista de un elemento
    if isinstance(data, list) and len(data) == 1:
        data = data[0]
    if not isinstance(data, dict):
        raise ValueError("la respuesta no es un objeto JSON")
    return data


def call_vision_model(jpeg_bytes: bytes, client: Optional[httpx.Client] = None) -> Tuple[dict, str]:
    """Pide el ticket a cada modelo configurado hasta que uno responda con JSON válido."""
    api_key = os.getenv("OPENROUTER_API_KEY", "").strip()
    models = vision_models()
    if not api_key or not models:
        raise VisionUnavailableError("lector por visión no configurado")

    base_url = os.getenv("OPENROUTER_BASE_URL", DEFAULT_BASE_URL).rstrip("/")
    # Presupuesto total para todos los modelos: debe quedar por debajo del timeout del
    # backend (INTELLIGENCE_TIMEOUT_SECS) para que aún haya tiempo de usar Tesseract.
    budget = float(os.getenv("OCR_VISION_TIMEOUT", "25"))
    deadline = time.monotonic() + budget
    jpeg_b64 = base64.b64encode(jpeg_bytes).decode()
    headers = {"Authorization": f"Bearer {api_key}", "X-Title": "Mercastats"}

    owns_client = client is None
    client = client or httpx.Client(timeout=budget)
    try:
        for model in models:
            remaining = deadline - time.monotonic()
            if remaining < MIN_ATTEMPT_SECONDS:
                logger.warning("Sin tiempo para más modelos de visión")
                break
            started = time.perf_counter()
            try:
                response = client.post(
                    f"{base_url}/chat/completions",
                    headers=headers,
                    json=_request_body(model, jpeg_b64),
                    timeout=remaining,
                )
                if response.status_code != 200:
                    logger.warning("Modelo de visión rechazó la petición | modelo={} | http={}", model, response.status_code)
                    continue
                payload = response.json()
                data = _parse_content(payload["choices"][0]["message"].get("content") or "")
            except httpx.HTTPError as exc:
                logger.warning("Fallo de red con el modelo de visión | modelo={} | tipo={}", model, type(exc).__name__)
                continue
            except (KeyError, IndexError, TypeError, ValueError) as exc:
                logger.warning("Respuesta ilegible del modelo de visión | modelo={} | tipo={}", model, type(exc).__name__)
                continue

            usage = payload.get("usage") or {}
            # Sin datos del ticket en los logs (ver docs/SECURITY_AUDIT.md): solo coste y tiempos.
            logger.info(
                "Ticket leído por visión | modelo={} | {:.1f}s | coste=${}",
                model,
                time.perf_counter() - started,
                usage.get("cost", "?"),
            )
            return data, model
    finally:
        if owns_client:
            client.close()

    raise VisionUnavailableError("ningún modelo de visión devolvió una respuesta válida")


# ============================================================================
# NORMALIZACIÓN
# ============================================================================


def _number(value) -> Optional[float]:
    if value is None or isinstance(value, bool):
        return None
    if isinstance(value, str):
        value = value.replace("€", "").strip()
        if "," in value:  # formato español: 1.234,56
            value = value.replace(".", "").replace(",", ".")
    try:
        return float(value)
    except (TypeError, ValueError):
        return None


def _text(value) -> Optional[str]:
    if not isinstance(value, str):
        return None
    value = re.sub(r"\s+", " ", value).strip()
    return value or None


def normalize_payment(value: Optional[str]) -> Optional[str]:
    """Método de pago con los valores que admite el backend."""
    value = (_text(value) or "").upper()
    if "EFECTIVO" in value:
        return "EFECTIVO"
    if "BIZUM" in value:
        return "BIZUM"
    if any(word in value for word in ("TARJ", "VISA", "MASTERCARD", "AMEX")):
        return "TARJETA BANCARIA"
    return None


def clean_product_name(name: str, cantidad: float, unidad: str) -> str:
    """Mayúsculas, espacios simples y sin la columna de cantidad si el modelo la coló en el nombre.

    El número inicial solo se quita si es la cantidad comprada (o el "1" de un producto a
    peso): hay productos que empiezan por un número, como "12 HUEVOS GRANDES-L".
    """
    name = re.sub(r"\s+", " ", name).strip().upper()
    match = CANTIDAD_EN_NOMBRE_RE.match(name)
    if not match or match.end() == len(name):
        return name
    prefijo = float(match.group().strip().replace(",", "."))
    if prefijo == cantidad or (unidad != "unidad" and prefijo == 1):
        return name[match.end():]
    return name


def _fecha(fecha: Optional[str], hora: Optional[str]) -> Tuple[Optional[str], Optional[datetime]]:
    """Fecha DD/MM/AAAA y, si la hora también se lee, el instante completo."""
    fecha = _text(fecha)
    if not fecha:
        return None, None
    try:
        datetime.strptime(fecha, "%d/%m/%Y")
    except ValueError:
        return None, None
    hora = _text(hora)
    if hora:
        try:
            return fecha, datetime.strptime(f"{fecha} {hora}", "%d/%m/%Y %H:%M")
        except ValueError:
            pass
    return fecha, None


def _products(raw: list) -> List[ParsedProduct]:
    productos: List[ParsedProduct] = []
    for item in raw if isinstance(raw, list) else []:
        if not isinstance(item, dict):
            continue
        nombre = _text(item.get("nombre"))
        precio_total = _number(item.get("precio_total"))
        if not nombre or precio_total is None:
            continue
        cantidad = _number(item.get("cantidad")) or 1.0
        if cantidad <= 0:
            cantidad = 1.0
        unidad = item.get("unidad") if item.get("unidad") in {"unidad", "kg", "l"} else "unidad"
        precio_unitario = _number(item.get("precio_unitario"))
        if precio_unitario is None or precio_unitario < 0:
            precio_unitario = round(precio_total / cantidad, 2)
        productos.append(
            ParsedProduct(
                nombre=clean_product_name(nombre, cantidad, unidad),
                cantidad=round(cantidad, 3),
                unidad=unidad,
                precio_unitario=round(precio_unitario, 2),
                precio_total=round(precio_total, 2),
            )
        )
    return productos


def _iva(raw: list) -> List[IvaBreakdown]:
    desglose: List[IvaBreakdown] = []
    for item in raw if isinstance(raw, list) else []:
        if not isinstance(item, dict):
            continue
        porcentaje = _number(item.get("porcentaje"))
        base = _number(item.get("base_imponible"))
        cuota = _number(item.get("cuota"))
        if None in (porcentaje, base, cuota):
            continue
        desglose.append(IvaBreakdown(porcentaje=porcentaje, base_imponible=round(base, 2), cuota=round(cuota, 2)))
    return desglose


def render_text(ticket: ParsedTicket) -> str:
    """Texto legible del ticket leído (sustituye al texto crudo que da un PDF)."""
    lines = [ticket.tienda or "MERCADONA"]
    if ticket.ubicacion:
        lines.append(ticket.ubicacion)
    fecha = ticket.fecha_hora.strftime("%d/%m/%Y %H:%M") if ticket.fecha_hora else ticket.fecha
    if fecha:
        lines.append(fecha + (f" OP:{ticket.numero_operacion}" if ticket.numero_operacion else ""))
    if ticket.numero_factura:
        lines.append(f"FACTURA SIMPLIFICADA: {ticket.numero_factura}")
    for p in ticket.productos:
        cantidad = f"{p.cantidad:g}" if p.unidad == "unidad" else f"{p.cantidad:g} {p.unidad}"
        lines.append(f"{cantidad} {p.nombre} {_euros(p.precio_total)}")
    if ticket.total is not None:
        lines.append(f"TOTAL (€) {_euros(ticket.total)}")
    return "\n".join(lines)


def _euros(value: float) -> str:
    return f"{value:.2f}".replace(".", ",")


def to_parsed_ticket(data: dict) -> Tuple[ParsedTicket, List[str]]:
    """Convierte el JSON del modelo en un ``ParsedTicket`` validado."""
    warnings: List[str] = []

    if data.get("es_ticket_mercadona") is False:
        raise TicketRejectedError("La imagen no parece un ticket de Mercadona.")

    numero_factura = _text(data.get("numero_factura"))
    if numero_factura and not NUMERO_FACTURA_RE.match(numero_factura):
        warnings.append("El número de factura leído no tiene el formato esperado; revísalo.")
        numero_factura = None

    fecha, fecha_hora = _fecha(data.get("fecha"), data.get("hora"))
    total = _number(data.get("total"))

    ticket = ParsedTicket(
        raw_text="",
        numero_factura=numero_factura,
        fecha=fecha,
        fecha_hora=fecha_hora,
        total=round(total, 2) if total is not None else None,
        tienda=_text(data.get("tienda")),
        ubicacion=_text(data.get("direccion")),
        metodo_pago=normalize_payment(data.get("metodo_pago")),
        numero_operacion=_text(data.get("numero_operacion")),
        productos=_products(data.get("productos")),
        iva_desglose=_iva(data.get("iva_desglose")),
    )
    assign_iva_to_products(ticket.productos, ticket.iva_desglose)
    ticket.raw_text = render_text(ticket)
    return ticket, warnings


def parse_ticket_photo(image: Image.Image, client: Optional[httpx.Client] = None) -> VisionResult:
    """Lee una foto de ticket con el modelo de visión configurado."""
    data, model = call_vision_model(to_model_jpeg(image), client=client)
    ticket, warnings = to_parsed_ticket(data)
    return VisionResult(ticket=ticket, model=model, warnings=warnings)
