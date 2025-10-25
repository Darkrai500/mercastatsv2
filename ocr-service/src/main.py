"""
FastAPI application para el worker OCR de Mercastats.

Endpoints:
    - GET /health: Health check
    - POST /process-ticket: Procesa un ticket PDF y devuelve datos estructurados
"""

import sys
from contextlib import asynccontextmanager
from dataclasses import asdict

from fastapi import FastAPI, HTTPException, status
from fastapi.middleware.cors import CORSMiddleware
from fastapi.responses import JSONResponse
from loguru import logger

from .models import HealthResponse, ProcessTicketRequest, ProcessTicketResponse
from .services import PDFParsingError, parse_ticket


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
        parsed_ticket = parse_ticket(request.pdf_b64)

        response = ProcessTicketResponse(
            ticket_id=request.ticket_id,
            raw_text=parsed_ticket.raw_text,
            numero_factura=parsed_ticket.numero_factura,
            fecha=parsed_ticket.fecha,
            fecha_hora=parsed_ticket.fecha_hora,
            total=parsed_ticket.total,
            tienda=parsed_ticket.tienda,
            ubicacion=parsed_ticket.ubicacion,
            metodo_pago=parsed_ticket.metodo_pago,
            numero_operacion=parsed_ticket.numero_operacion,
            productos=[asdict(prod) for prod in parsed_ticket.productos],
            iva_desglose=[asdict(item) for item in parsed_ticket.iva_desglose],
        )

        logger.success(
            "Ticket procesado | factura=%s | fecha=%s | total=%s | productos=%s",
            parsed_ticket.numero_factura,
            parsed_ticket.fecha_hora.isoformat(timespec="minutes")
            if parsed_ticket.fecha_hora
            else parsed_ticket.fecha,
            parsed_ticket.total,
            len(parsed_ticket.productos),
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

