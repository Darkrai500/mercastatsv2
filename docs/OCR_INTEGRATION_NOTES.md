# OCR Integration Notes

## Context

Se ha refactorizado el worker de OCR en Python para que el backend de Rust pueda reutilizar directamente su lógica sin levantar un servidor FastAPI independiente. El backend ahora expone un endpoint que llama a la librería de Python mediante PyO3 y ejecuta el procesamiento dentro del mismo proceso de `cargo run`.

## Cambios clave

- **Python**
  - Nuevo módulo `ocr-service/src/processor.py` que centraliza la generación de la respuesta (`process_ticket_response`, `process_ticket_payload`, `process_ticket_json`).
  - `ocr-service/src/main.py` ahora usa ese módulo compartido para construir la respuesta y dejo listas las trazas de éxito con los datos provenientes del modelo Pydantic.
  - `ocr-service/src/__init__.py` exporta los helpers para que PyO3 pueda importarlos con `import ocr_service`.

- **Rust**
  - Dependencia `pyo3` añadida en `backend/Cargo.toml`.
  - Nuevo módulo `backend/src/services/ocr.rs`:
    - Prepara el `sys.path` para incluir `ocr-service/src`.
    - Llama a `process_ticket_json` y deserializa el JSON en structs equivalentes Rust (`ProcessTicketResponse`, `TicketProduct`, etc.).
    - Aísla errores de parsing (`PDFParsingError`) y fallos internos de Python.
  - Reexportes en `backend/src/services/mod.rs` para consumir el servicio desde rutas.
  - DTOs de entrada/salida en `backend/src/schema/ocr.rs` reutilizando las estructuras del servicio.
  - Nuevo router `backend/src/routes/ocr.rs` con `POST /api/ocr/process`.
  - `backend/src/main.rs` anida el router OCR bajo `/api/ocr`.

## Cómo funciona el flujo ahora

1. El frontend envía `ticket_id`, `file_name` y `pdf_b64` a `POST /api/ocr/process`.
2. Axum valida el payload y llama al servicio `process_ticket_ocr`.
3. El servicio entra al intérprete de Python (bloqueante) en un thread mediante `spawn_blocking`.
4. Python decodifica el PDF, aplica reglas del parser y devuelve el JSON serializado.
5. El backend responde con la misma estructura que producía el worker FastAPI.

## Requisitos

- Python debe estar disponible en el entorno que ejecuta `cargo run` (PyO3 usa el intérprete del sistema).
- El directorio `ocr-service/src` permanece en el repo; no hay que instalarlo como paquete externo.
- Dependencias de Python siguen en `ocr-service/requirements.txt` (ejecutar `pip install -r` cuando toque desplegar el backend).

## Próximos pasos sugeridos

1. Añadir pruebas de integración que llamen al endpoint y validen la respuesta con PDFs reales (las fixtures pueden residir en `ocr-service/test_response.json` o nuevos archivos en `docs/`).
2. Gestionar la inicialización del intérprete en producción (por ejemplo, logs adicionales o health check que valide `process_ticket_json` con un PDF sintético).
3. Revisar rendimiento: si el parsing tarda mucho, considerar colas o workers dedicados, pero manteniendo la interfaz unificada.
