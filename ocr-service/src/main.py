"""
FastAPI application para el worker OCR de Mercastats.

Endpoints:
    - GET /health: Health check
    - POST /process-ticket: Procesa un ticket PDF y devuelve datos estructurados
"""

import sys
from contextlib import asynccontextmanager
from fastapi import FastAPI, HTTPException, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from loguru import logger

from .models import HealthResponse, ProcessTicketRequest
from .processor import PDFParsingError, process_ticket_response


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
# LIFECYCLE EVENTS
# ============================================================================


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Gestiona el ciclo de vida de la aplicacion FastAPI."""
    logger.info("Iniciando OCR Service...")
    logger.info("Version: 1.0.0")
    logger.info("Dependencias: pdfplumber")
    yield
    logger.info("Deteniendo OCR Service...")


# ============================================================================
# APLICACION FASTAPI
# ============================================================================

app = FastAPI(
    title="Mercastats OCR Service",
    description="Worker Python para procesamiento de tickets PDF de Mercadona",
    version="1.0.0",
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
    return {"message": "OCR Service running. Check /health for status."}


@app.get("/health", response_model=HealthResponse, tags=["Health"])
async def health_check():
    """Health check endpoint."""
    logger.info("Health check requested")
    return HealthResponse(status="ok", service="ocr-service", version="1.0.0")


@app.post("/process-ticket", response_model=ProcessTicketResponse, tags=["OCR"])
async def process_ticket(request: ProcessTicketRequest):
    """
    Procesa un ticket PDF y extrae informacion estructurada.
    """
    logger.info("Procesando ticket %s | archivo=%s", request.ticket_id, request.file_name)

    try:
        response = process_ticket_response(
            ticket_id=request.ticket_id,
            file_name=request.file_name,
            pdf_b64=request.pdf_b64,
        )

        fecha_repr = (
            response.fecha_hora.isoformat(timespec="minutes")
            if response.fecha_hora
            else response.fecha
        )

        logger.success(
            "Ticket procesado | factura=%s | fecha=%s | total=%s | productos=%s",
            response.numero_factura,
            fecha_repr,
            response.total,
            len(response.productos),
        )

        return response

    except PDFParsingError as exc:
        logger.error("Error de parsing: %s", exc)
        raise HTTPException(
            status_code=status.HTTP_422_UNPROCESSABLE_ENTITY,
            detail=f"No se pudo procesar el PDF: {exc}",
        ) from exc
    except Exception as exc:
        logger.exception("Error inesperado: %s", exc)
        raise HTTPException(
            status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
            detail=f"Error interno del servidor: {exc}",
        ) from exc


# ============================================================================
# EXCEPTION HANDLERS
# ============================================================================


@app.exception_handler(Exception)
async def global_exception_handler(request, exc):
    """Handler global para excepciones no capturadas."""
    logger.exception("Excepcion no capturada: %s", exc)
    return JSONResponse(
        status_code=status.HTTP_500_INTERNAL_SERVER_ERROR,
        content={"detail": "Error interno del servidor"},
    )


# ============================================================================
# MAIN (para desarrollo)
# ============================================================================


if __name__ == "__main__":
    import uvicorn

    uvicorn.run(
        "main:app",
        host="127.0.0.1",
        port=9000,
        reload=True,
        log_level="info",
    )
