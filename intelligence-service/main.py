"""
FastAPI application para Mercastats Intelligence Service.
Combina funcionalidades de OCR (procesamiento de tickets) y Predicción (ML).

Endpoints:
    - GET /health: Health check
    - POST /ocr/process: Procesa un ticket PDF y devuelve datos estructurados
    - POST /predict/next: Predice la próxima compra
"""

import sys
from contextlib import asynccontextmanager
from typing import List, Optional
from datetime import datetime, timedelta

from fastapi import FastAPI, HTTPException, Request, status
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
# MODELOS ML (Definidos aquí por simplicidad o migrar a models.py si crece)
# ============================================================================

class TicketFeature(BaseModel):
    numero_factura: Optional[str]
    fecha_hora: Optional[str] # ISO format
    total: Optional[float]
    day_of_week: int
    day_of_month: int
    hour_of_day: int
    days_since_last_shop: float
    total_last_30d: float
    tickets_last_30d: int
    is_payday_week: bool
    # Targets for training (optional in inference)
    target_days_until_next: Optional[float] = None

class PredictRequest(BaseModel):
    user_id: str
    current_date: str
    features_now: TicketFeature
    history_features: List[TicketFeature]

class PredictionResponse(BaseModel):
    timestamp: str
    time_window_label: str
    estimated_total: float
    confidence: float
    suggested_products: List[dict]

# ============================================================================
# LIFECYCLE EVENTS
# ============================================================================

predictor = None

@asynccontextmanager
async def lifespan(app: FastAPI):
    """Gestiona el ciclo de vida de la aplicacion FastAPI."""
    global predictor
    logger.info("Iniciando Intelligence Service (OCR + ML)...")
    logger.info("Cargando modelo de predicción...")
    predictor = ShoppingPredictor.load()
    logger.info(f"Modelo cargado: {predictor.is_trained}")
    yield
    logger.info("Deteniendo Intelligence Service...")

# ============================================================================
# APLICACION FASTAPI
# ============================================================================

app = FastAPI(
    title="Mercastats Intelligence Service",
    description="Servicio unificado para OCR y Predicción ML",
    version="2.0.0",
    lifespan=lifespan,
)

app.add_middleware(
    CORSMiddleware,
    allow_origins=["*"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# ============================================================================
# ENDPOINTS
# ============================================================================

@app.get("/", include_in_schema=False)
async def root():
    """Redirige a /health."""
    return {"message": "Intelligence Service running. Check /health for status."}

@app.get("/health", response_model=HealthResponse, tags=["Health"])
async def health_check():
    """Health check endpoint."""
    logger.info("Health check requested")
    return HealthResponse(
        status="ok", 
        service="intelligence-service", 
        version="2.0.0"
    )

# --- OCR ENDPOINTS ---

@app.post("/ocr/process", response_model=ProcessTicketResponse, tags=["OCR"])
async def process_ticket(request: ProcessTicketRequest):
    """
    Procesa un ticket PDF y extrae informacion estructurada.
    """
    logger.info(
        "Procesando ticket {} | archivo={}",
        request.ticket_id,
        request.file_name,
    )

    try:
        response = process_ticket_response(
            ticket_id=request.ticket_id,
            file_name=request.file_name,
            pdf_b64=request.file_content_b64,
        )

        fecha_repr = (
            response.fecha_hora.isoformat(timespec="minutes")
            if response.fecha_hora
            else response.fecha
        )

        logger.success(
            "Ticket procesado | factura={} | fecha={} | total={} | productos={}",
            response.numero_factura,
            fecha_repr,
            response.total,
            len(response.productos),
        )

        return response

    except PDFParsingError as exc:
        logger.error("Error de parsing: {}", exc)
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail=f"No se pudo procesar el PDF: {exc}",
        ) from exc
    except Exception as exc:
        logger.exception("Error inesperado: {}", exc)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error interno del servidor: {exc}",
        ) from exc

# --- ML ENDPOINTS ---

@app.post("/predict/next", response_model=dict, tags=["ML"])
async def predict_next(request: PredictRequest):
    global predictor
    if predictor is None:
        predictor = ShoppingPredictor.load()

    # 1. Train/Update model with history (in a real app, this might be async or scheduled)
    if request.history_features:
        df_history = pd.DataFrame([vars(f) for f in request.history_features])
        predictor.train(df_history)
    
    if not predictor.is_trained:
        raise HTTPException(status_code=503, detail="Model not trained yet")

    # 2. Predict
    current_features = vars(request.features_now)
    model_input = {
        'day_of_week': current_features['day_of_week'],
        'hour_of_day': current_features['hour_of_day'],
        'days_since_last_shop': current_features['days_since_last_shop'],
        'total_last_30d': current_features['total_last_30d'],
        'tickets_last_30d': current_features['tickets_last_30d'],
        'is_payday_week': current_features['is_payday_week']
    }
    
    result = predictor.predict_next_visit(model_input)
    
    # 3. Format response
    current_dt = datetime.fromisoformat(request.current_date)
    
    days_until = result['days_until']
    if pd.isna(days_until):
        logger.warning("Prediccion de dias es NaN, usando valor por defecto (7 dias)")
        days_until = 7.0
        
    predicted_date = current_dt + timedelta(days=days_until)
    predicted_date = predicted_date.replace(hour=result['predicted_hour'], minute=0, second=0)
    
    # Determine learning mode
    learning_mode = False
    if request.history_features and len(request.history_features) < 15:
        learning_mode = True

    return {
        "prediction": {
            "timestamp": predicted_date.isoformat(),
            "time_window_label": f"Estimated around {result['predicted_hour']}:00",
            "estimated_total": round(result['predicted_spend'], 2),
            "confidence": 0.85, # Placeholder
            "suggested_products": [], # Placeholder for now
            "learning_mode": learning_mode
        }
    }

# ============================================================================
# EXCEPTION HANDLERS
# ============================================================================

@app.exception_handler(Exception)
async def global_exception_handler(request: Request, exc: Exception):
    """Handler global para excepciones no capturadas."""
    logger.exception("Excepcion no capturada: {}", exc)
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
