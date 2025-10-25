# Plan de Implementación – Worker Python (Procesamiento PDF)

Última actualización: 25/10/2025

## Objetivo

Implementar un worker en Python responsable de procesar tickets en formato PDF subidos desde el frontend. En esta primera fase (MVP) el backend enviará el PDF al worker, este extraerá información básica y el backend solo mostrará por terminal (logging) lo extraído para depuración.

## Contexto Actual del Proyecto

- Frontend (`frontend/src/pages/upload.rs`) ya permite seleccionar y subir archivos; usa `upload_ticket` (`frontend/src/api/tickets.rs`) contra `POST /api/tickets/upload`.
- Backend (`backend/src/main.rs`) expone rutas de auth y health; no existe aún el módulo de tickets ni integración con workers.
- Base de datos (`sql/schema/schema.sql`) incluye tablas clave:
  - `compras` (metadatos del ticket, PK `numero_factura`).
  - `tickets_pdf` (contenido PDF binario, FK `numero_factura` → `compras`).

## Alcance Fase 1 (MVP)

- Soportar únicamente PDFs (no imágenes aún).
- Flujo síncrono: el backend llama al worker durante el upload y loguea el resultado.
- No se persiste lo extraído; solo logging de depuración.

## Diseño del Worker (Python)

- Stack: FastAPI + Uvicorn.
- Dependencias iniciales: `fastapi`, `uvicorn[standard]`, `pydantic`, `python-multipart`, `pdfplumber`, `loguru`.
- Estructura propuesta:

```
ocr-service/
├── requirements.txt
├── README.md
└── src/
    ├── main.py               # FastAPI app, endpoint /process-ticket
    ├── services/
    │   └── pdf_parser.py     # Lógica de extracción PDF
    ├── models.py             # Pydantic models (request/response)
    └── constants.py          # Regex/constantes
```

- Endpoint: `POST /process-ticket`
  - Request JSON:
    - `ticket_id: str` (ID provisional generado por backend)
    - `file_name: str`
    - `pdf_b64: str` (contenido PDF en base64)
  - Respuesta JSON:
    - `ticket_id: str`
    - `raw_text: str` (texto completo extraído)
    - `numero_factura: Optional[str]`
    - `fecha: Optional[str]` (formato dd/mm/yyyy si se detecta)
    - `total: Optional[float]`

- Extracción PDF (en `pdf_parser.py`):
  1. Decodificar base64 → `BytesIO`.
  2. Extraer texto con `pdfplumber` (iterando páginas, uniendo con saltos de línea).
  3. Aplicar heurísticas regex para Mercadona:
     - Número de factura: `\b\d{4}-\d{3}-\d{6}\b`
     - Fecha: `\b\d{2}/\d{2}/\d{4}\b`
     - Total: `\bTOTAL\s+([0-9]+,[0-9]{2})\b` (convertir coma a punto)
  4. Retornar `raw_text` y campos detectados (si alguno no aparece, devolver `null`).
  5. Logging con Loguru para trazabilidad: `info("ticket={id} total={total} factura={...}")`.

## Cambios en Backend (Rust)

1. Configuración
   - Añadir `WORKER_URL` al `.env` (p. ej. `http://127.0.0.1:9000`).
   - Extender `backend/src/config.rs` con `pub worker_url: String` (con default opcional) y exponerlo en `AppConfig`.

2. Rutas y servicios
   - Crear `backend/src/routes/tickets.rs` con endpoint `POST /api/tickets/upload` que acepte `multipart/form-data` (`axum::extract::Multipart`).
   - Crear `backend/src/services/tickets.rs` que orqueste:
     - Validar y leer el archivo del multipart (solo PDFs; validar tamaño razonable).
     - Persistir en BD (mínimo `tickets_pdf`; si aún no hay `numero_factura`, usar un `ticket_id` UUID temporal solo para correlación de logs).
     - Construir payload para el worker (base64 del PDF) y llamar vía `reqwest::Client` a `POST {WORKER_URL}/process-ticket`.
     - Loggear el resultado con `tracing::info!` (no persistir por ahora).

3. Capa DB (SQLx)
   - `backend/src/db/tickets.rs` con funciones:
     - `insert_ticket_pdf(ticket_id/numero_factura, nombre_archivo, bytes_len, bytes)` para `tickets_pdf` (si `numero_factura` aún no se conoce, usar una tabla staging temporal o un campo `numero_factura` provisional; para MVP se puede usar un `ticket_id` independiente y diferir el `INSERT` en `compras`).
   - Nota: el schema actual exige `numero_factura` válido para `tickets_pdf`. Para el MVP, se puede:
     - Opción A (simple para logs): no persistir todavía en `tickets_pdf`, y solo enviar al worker y loggear.
     - Opción B (recomendada a corto plazo): crear tabla `tickets_ingest_temp` (id UUID, pdf BYTEA, filename, size, created_at). Luego, cuando el worker devuelva `numero_factura`, mover a `compras`/`tickets_pdf`.

4. Modelos y DTOs
   - `backend/src/models/ticket.rs`: `UploadResponse`, `WorkerRequest`, `WorkerResponse` (serde serialize/deserialize).

5. Wiring en `main.rs`
   - Registrar `.nest("/api/tickets", routes::tickets_router(state))`.

6. Errores y logging
   - Mapear errores de parseo/HTTP a 422/502 usando `thiserror`/`anyhow` y `IntoResponse` ya existente en `error.rs`.
   - `tracing` para logs claros: `ticket_id`, `filename`, tamaños y resumen de extracción.

## Flujo End-to-End (MVP)

1. Frontend envía `multipart/form-data` con el PDF a `POST /api/tickets/upload`.
2. Backend valida token, lee el archivo y genera `ticket_id` (UUID) para correlación.
3. Backend (MVP):
   - Opción A: no guarda en BD aún; serializa a base64 y llama al worker.
   - Opción B (mejor pronto): guarda en `tickets_ingest_temp` y luego llama al worker.
4. Worker extrae texto y heurísticas (factura, fecha, total) y responde JSON.
5. Backend hace `tracing::info!(ticket_id, factura, fecha, total, sample=first_n_chars(raw_text))`.
6. Backend responde al frontend `{ ticket_id, message: "Ticket subido con éxito" }`.

## Operativa Local

- Variables de entorno:
  - Backend: `WORKER_URL=http://127.0.0.1:9000`.
  - Worker: no requiere secrets en MVP.
- Arranque worker:
  - `cd ocr-service && python -m venv .venv && .venv/Scripts/activate` (Windows) o `source .venv/bin/activate` (Unix).
  - `pip install -r requirements.txt`.
  - `uvicorn src.main:app --reload --port 9000`.

## Testing

- Worker (Python):
  - Tests unitarios de `pdf_parser.py` con PDFs de prueba (fixtures) y asserts de regex.
  - Test de contrato del endpoint `/process-ticket` usando `httpx.AsyncClient` o `pytest` + `fastapi.testclient`.

- Backend (Rust):
  - Test de integración para `POST /api/tickets/upload` simulando multipart; mock del worker (p. ej. `wiremock` o feature flag para inyectar un `Client` falso).
  - Verificar que se hace `tracing::info!` con campos esperados.

## Roadmap Posterior

1. Persistir datos extraídos (`compras`, `compras_productos`, `historico_precios`).
2. Manejar imágenes (PDF escaneado → OCR con Tesseract) y layout parsing.
3. Idempotencia: evitar duplicados por `numero_factura`.
4. Procesamiento asíncrono/cola (RabbitMQ/Redis) para desacoplar upload del parsing.
5. Dockerización del worker y `docker-compose` para todo el stack.

## Contrato Worker ↔ Backend (resumen)

- Request (JSON):
```json
{
  "ticket_id": "UUID",
  "file_name": "ticket_2025_10_25.pdf",
  "pdf_b64": "<base64>"
}
```

- Response (JSON):
```json
{
  "ticket_id": "UUID",
  "raw_text": "...",
  "numero_factura": "1234-567-890123",
  "fecha": "25/10/2025",
  "total": 42.35
}
```

Notas:
- `raw_text` puede ser grande; para logging, truncar a los primeros N caracteres.
- Campos heurísticos son opcionales; no fallar si no se detectan.

