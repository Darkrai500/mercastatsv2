"""
FastAPI application para Mercastats Intelligence Service.
Combina funcionalidades de OCR (procesamiento de tickets) y Predicción (ML).

Endpoints:
    - GET /health: Health check
    - POST /ocr/process: Procesa un ticket PDF y devuelve datos estructurados
    - POST /predict/next: Predice la próxima compra
"""

import os
import secrets
import sys
from contextlib import asynccontextmanager
from datetime import date, datetime, timedelta
from typing import List, Optional
from zoneinfo import ZoneInfo, ZoneInfoNotFoundError

from fastapi import Depends, FastAPI, Header, HTTPException, Request, status
from fastapi.exceptions import RequestValidationError
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from loguru import logger
from pydantic import BaseModel, Field

# Importaciones de OCR
from models import (
    HealthResponse,
    ProcessTicketRequest,
    ProcessTicketResponse,
)
from processor import PDFParsingError, process_ticket_response

# Importaciones de ML
from predictor import Prediction, ProductLine, Ticket, predict_next_purchase

# ============================================================================
# CONFIGURACION DE LOGGING
# ============================================================================

logger.remove()
logger.add(
    sys.stderr,
    format="<green>{time:YYYY-MM-DD HH:mm:ss}</green> | <level>{level: <8}</level> | "
    "<cyan>{name}</cyan>:<cyan>{function}</cyan>:<cyan>{line}</cyan> - <level>{message}</level>",
    level="INFO",
)

# ============================================================================
# MODELOS ML
# ============================================================================

MAX_HISTORY_TICKETS = 1000
MAX_PRODUCT_LINES = 20000


class TicketFeature(BaseModel):
    """
    Un ticket del historial del usuario.

    Solo ``fecha_hora`` y ``total`` alimentan el modelo: el resto de características
    (día de la semana, hora, huecos entre visitas...) se derivan aquí a partir de la
    fecha para que existan en un único sitio con una única convención. Las versiones
    anteriores del backend enviaban también esos campos derivados; se aceptan y se
    ignoran para no romper la compatibilidad.
    """

    fecha_hora: Optional[str] = None  # ISO 8601, hora local del ticket
    total: Optional[float] = None


class ProductPurchase(BaseModel):
    """Una línea de producto de un ticket pasado."""

    name: str
    fecha_hora: str  # ISO 8601, hora local del ticket
    quantity: float
    unit_price: float


class PredictRequest(BaseModel):
    user_id: Optional[str] = None  # no se usa: el servicio no guarda estado por usuario
    current_date: datetime
    history_features: List[TicketFeature] = Field(default_factory=list, max_length=MAX_HISTORY_TICKETS)
    product_history: List[ProductPurchase] = Field(default_factory=list, max_length=MAX_PRODUCT_LINES)


# ============================================================================
# LIFECYCLE EVENTS
# ============================================================================


@asynccontextmanager
async def lifespan(app: FastAPI):
    logger.info("Iniciando Intelligence Service (OCR + ML)...")
    yield
    logger.info("Deteniendo Intelligence Service...")


# ============================================================================
# APP
# ============================================================================

app = FastAPI(
    title="Mercastats Intelligence Service",
    description="Servicio unificado para OCR y Predicción ML",
    version="2.0.0",
    lifespan=lifespan,
)

cors_origins = [
    origin.strip()
    for origin in os.getenv(
        "CORS_ORIGINS",
        "http://localhost:3000,http://localhost:8080",
    ).split(",")
    if origin.strip()
]

app.add_middleware(
    CORSMiddleware,
    allow_origins=cors_origins,
    allow_credentials=False,
    allow_methods=["GET", "POST", "OPTIONS"],
    allow_headers=["Authorization", "Content-Type", "X-API-Key"],
)


# ============================================================================
# ENDPOINTS
# ============================================================================

async def require_internal_api_key(
    x_api_key: Optional[str] = Header(default=None),
) -> None:
    """Exige la clave compartida cuando está configurada en el entorno."""
    expected = os.getenv("INTELLIGENCE_API_KEY", "").strip()
    if expected and (
        x_api_key is None or not secrets.compare_digest(x_api_key, expected)
    ):
        raise HTTPException(status_code=401, detail="Credencial interna inválida")


@app.get("/", include_in_schema=False, dependencies=[Depends(require_internal_api_key)])
async def root():
    return {"message": "Intelligence Service running. Check /health for status."}


@app.get(
    "/health",
    response_model=HealthResponse,
    tags=["Health"],
    dependencies=[Depends(require_internal_api_key)],
)
async def health_check():
    logger.info("Health check requested")
    return HealthResponse(status="ok", service="intelligence-service", version="2.0.0")


# --- OCR ENDPOINTS ---


from processor import ImageParsingError

@app.post(
    "/ocr/process",
    response_model=ProcessTicketResponse,
    tags=["OCR"],
    dependencies=[Depends(require_internal_api_key)],
)
async def process_ticket(request: ProcessTicketRequest):
    logger.info("Procesando ticket | mime=%s", request.mime_type)

    try:
        response = process_ticket_response(
            ticket_id=request.ticket_id,
            file_name=request.file_name,
            pdf_b64=request.file_content_b64,
            mime_type=request.mime_type,
        )

        logger.success(
            "Ticket procesado | productos=%s | avisos=%s",
            len(response.productos),
            len(response.warnings),
        )

        return response

    except (PDFParsingError, ImageParsingError) as exc:
        logger.warning("El ticket no superó la validación del parser")
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail="No se pudo procesar el archivo",
        ) from exc
    except Exception as exc:
        logger.exception("Error inesperado al procesar un ticket")
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail="Error interno del servidor",
        ) from exc


# --- ML ENDPOINTS ---


WEEKDAY_NAMES = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"]


def local_timezone() -> ZoneInfo:
    """Zona horaria de las tiendas (los tickets guardan la hora local de pared)."""
    name = os.getenv("APP_TIMEZONE", "Europe/Madrid")
    try:
        return ZoneInfo(name)
    except ZoneInfoNotFoundError:
        logger.warning("APP_TIMEZONE no es una zona válida; se usa Europe/Madrid")
        return ZoneInfo("Europe/Madrid")


def to_local_naive(value: datetime, tz: ZoneInfo) -> datetime:
    """Hora local de pared sin zona; los instantes con zona (p. ej. UTC) se convierten."""
    if value.tzinfo is not None:
        value = value.astimezone(tz).replace(tzinfo=None)
    return value


def _parse_local(value: Optional[str], tz: ZoneInfo) -> Optional[datetime]:
    if not value:
        return None
    try:
        return to_local_naive(datetime.fromisoformat(value), tz)
    except ValueError:
        return None


def parse_tickets(rows: List[TicketFeature], tz: ZoneInfo) -> List[Ticket]:
    tickets = []
    for row in rows:
        when = _parse_local(row.fecha_hora, tz)
        if when is None or row.total is None or row.total < 0:
            continue
        tickets.append(Ticket(when=when, total=float(row.total)))
    return tickets


def parse_product_lines(rows: List[ProductPurchase], tz: ZoneInfo) -> List[ProductLine]:
    lines = []
    for row in rows:
        when = _parse_local(row.fecha_hora, tz)
        if when is None or row.quantity <= 0 or row.unit_price < 0 or not row.name:
            continue
        lines.append(ProductLine(row.name, when, float(row.quantity), float(row.unit_price)))
    return lines


def _clock(hour: float) -> str:
    total_minutes = int(round(hour * 60))
    return f"{total_minutes // 60:02d}:{total_minutes % 60:02d}"


def describe_day(target: date, today: date) -> str:
    weekday = WEEKDAY_NAMES[target.weekday()]
    diff = (target - today).days
    if diff <= 0:
        return f"hoy ({weekday})"
    if diff == 1:
        return f"mañana ({weekday})"
    same_week = target - timedelta(days=target.weekday()) == today - timedelta(days=today.weekday())
    if diff < 7 and same_week:
        return f"este {weekday}"
    return f"el próximo {weekday}"


def build_prediction_payload(prediction: Prediction, now: datetime, tz: ZoneInfo) -> dict:
    """Da forma de contrato JSON (el que consume el backend Rust) a una predicción."""
    day_label = describe_day(prediction.when.date(), now.date())
    clock = prediction.when.hour + prediction.when.minute / 60.0
    return {
        "timestamp": prediction.when.replace(tzinfo=tz).isoformat(),
        "time_window_label": f"{day_label}, alrededor de las {_clock(clock)}",
        "time_window_range": f"{_clock(prediction.window_start)} - {_clock(prediction.window_end)}",
        "day_label": day_label,
        "estimated_total": prediction.total,
        "estimated_total_min": prediction.total_min,
        "estimated_total_max": prediction.total_max,
        "confidence": prediction.confidence,
        "suggested_products": [
            {
                "name": p.name,
                "probability": p.probability,
                "price_estimation": p.price_estimation,
                "reason": p.reason,
            }
            for p in prediction.products
        ],
        "learning_mode": prediction.learning_mode,
    }


# Se declara síncrona a propósito: el cálculo es CPU puro y FastAPI la ejecuta en su
# pool de hilos en lugar de bloquear el bucle de eventos.
@app.post(
    "/predict/next",
    response_model=dict,
    tags=["ML"],
    dependencies=[Depends(require_internal_api_key)],
)
def predict_next(request: PredictRequest):
    tz = local_timezone()
    now = to_local_naive(request.current_date, tz)

    prediction = predict_next_purchase(
        parse_tickets(request.history_features, tz),
        parse_product_lines(request.product_history, tz),
        now,
    )

    # Sin identificadores, importes ni productos en los logs (ver docs/SECURITY_AUDIT.md).
    logger.info(
        "Predicción generada | visitas={} | aprendiendo={}",
        prediction.visits_used,
        prediction.learning_mode,
    )
    return {"prediction": build_prediction_payload(prediction, now, tz)}


# ============================================================================
# EXCEPTION HANDLERS
# ============================================================================


@app.exception_handler(RequestValidationError)
async def validation_exception_handler(_request: Request, exc: RequestValidationError):
    # Por defecto FastAPI repite en la respuesta el valor recibido (campo "input"), que aquí
    # puede ser un dato de compra o un PDF entero, y acabaría en los logs de quien llama.
    errors = [
        {"loc": list(error.get("loc", ())), "msg": error.get("msg", ""), "type": error.get("type", "")}
        for error in exc.errors()
    ]
    return JSONResponse(status_code=422, content={"detail": errors})


@app.exception_handler(Exception)
async def global_exception_handler(_request: Request, exc: Exception):
    logger.error("Excepcion no capturada | tipo=%s", type(exc).__name__)
    return JSONResponse(
        status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
        content={"detail": "Error interno del servidor"},
    )


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        "main:app",
        host="127.0.0.1",
        port=8001,
        reload=True,
        log_level="info",
    )
