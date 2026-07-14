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
from typing import List, Optional
from datetime import datetime, timedelta

from fastapi import Depends, FastAPI, Header, HTTPException, Request, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from loguru import logger
from pydantic import BaseModel
import pandas as pd

# Importaciones de OCR
from models import (
    HealthResponse,
    ProcessTicketRequest,
    ProcessTicketResponse,
)
from processor import PDFParsingError, process_ticket_response

# Importaciones de ML
from predictor import ShoppingPredictor

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


class TicketFeature(BaseModel):
    numero_factura: Optional[str]
    fecha_hora: Optional[str]  # ISO format
    total: Optional[float]
    day_of_week: int
    day_of_month: int
    hour_of_day: int
    days_since_last_shop: float
    total_last_30d: float
    tickets_last_30d: int
    is_payday_week: bool
    target_days_until_next: Optional[float] = None


class PredictRequest(BaseModel):
    user_id: str
    current_date: str
    features_now: TicketFeature
    history_features: List[TicketFeature]


class PredictionResponse(BaseModel):
    timestamp: str
    time_window_label: str
    time_window_range: str
    day_label: str
    estimated_total: float
    estimated_total_min: float
    estimated_total_max: float
    confidence: float
    suggested_products: List[dict]


# ============================================================================
# LIFECYCLE EVENTS
# ============================================================================

predictor = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    global predictor
    logger.info("Iniciando Intelligence Service (OCR + ML)...")
    predictor = ShoppingPredictor.load()
    logger.info("Modelo cargado en arranque | is_trained=%s", predictor.is_trained)
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


@app.post(
    "/predict/next",
    response_model=dict,
    tags=["ML"],
    dependencies=[Depends(require_internal_api_key)],
)
async def predict_next(request: PredictRequest):
    global predictor
    if predictor is None:
        predictor = ShoppingPredictor.load()
        logger.info("Predictor inicializado en /predict/next | is_trained=%s", predictor.is_trained)

    # 1. Entrenamiento/incremental
    if request.history_features:
        df_history = pd.DataFrame([vars(f) for f in request.history_features])
        logger.info("Entrenando con %s tickets historicos", len(df_history))
        predictor.train(df_history)
    else:
        logger.warning("Sin historial para entrenar (se mantiene modelo previo)")

    if not predictor.is_trained:
        logger.error("Modelo no entrenado; abortando prediccion")
        raise HTTPException(status_code=503, detail="Model not trained yet")

    # 2. Predicción
    current_features = vars(request.features_now)
    logger.info("Prediccion solicitada | history=%s", len(request.history_features))
    model_input = {
        "day_of_week": current_features["day_of_week"],
        "hour_of_day": current_features["hour_of_day"],
        "days_since_last_shop": current_features["days_since_last_shop"],
        "total_last_30d": current_features["total_last_30d"],
        "tickets_last_30d": current_features["tickets_last_30d"],
        "is_payday_week": current_features["is_payday_week"],
    }

    result = predictor.predict_next_visit(model_input)
    logger.info("Prediccion completada")

    # 3. Formatear respuesta
    current_dt = datetime.fromisoformat(request.current_date)

    days_until = result["days_until"]
    if pd.isna(days_until):
        logger.warning("Prediccion de dias es NaN, usando valor por defecto (7 dias)")
        days_until = 7.0

    predicted_date = current_dt + timedelta(days=days_until)
    predicted_date = predicted_date.replace(hour=result["predicted_hour"], minute=0, second=0)

    day_diff = (predicted_date.date() - current_dt.date()).days
    weekday_names = ["lunes", "martes", "miércoles", "jueves", "viernes", "sábado", "domingo"]
    weekday = weekday_names[predicted_date.weekday()]
    if day_diff == 0:
        day_label = f"hoy ({weekday})"
    elif day_diff == 1:
        day_label = f"mañana ({weekday})"
    elif day_diff < 7:
        day_label = f"este {weekday}"
    else:
        day_label = f"el próximo {weekday}"

    start_hour = result["predicted_hour"]
    end_hour = min(start_hour + 2, 23)
    time_window_range = f"{start_hour:02d}:00 - {end_hour:02d}:00"

    base_total = max(result["predicted_spend"], 0.0)
    estimated_min = round(base_total * 0.9, 2)
    estimated_max = round(base_total * 1.1, 2)

    learning_mode = False
    if request.history_features and len(request.history_features) < 15:
        learning_mode = True

    # Sin placeholders: el backend inyectará productos reales
    suggested_products: list[dict] = []

    return {
        "prediction": {
            "timestamp": predicted_date.isoformat(),
            "time_window_label": f"{day_label}, alrededor de las {start_hour:02d}:00",
            "time_window_range": time_window_range,
            "day_label": day_label,
            "estimated_total": round(base_total, 2),
            "estimated_total_min": estimated_min,
            "estimated_total_max": estimated_max,
            "confidence": 0.85,
            "suggested_products": suggested_products,
            "learning_mode": learning_mode,
        }
    }


# ============================================================================
# EXCEPTION HANDLERS
# ============================================================================


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
