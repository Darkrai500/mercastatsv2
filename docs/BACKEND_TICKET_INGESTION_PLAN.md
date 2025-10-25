# Backend Ticket Ingestion Plan

## 1. Objective

Ingest the structured data produced by the embedded Python OCR worker into the Mercastats PostgreSQL schema using the Rust backend. The scope covers data flow from `POST /api/ocr/process` until the relevant tables (`compras`, `compras_productos`, `productos`, `historico_precios`, `tickets_pdf`) are populated, including validation, persistence, error handling, and observability.

## 2. Current State Summary

- The request payload (`ticket_id`, `file_name`, `pdf_b64`) is validated in `backend/src/schema/ocr.rs` and forwarded to `services::ocr::process_ticket`.
- The Python worker (`ocr-service/src/processor.py`) returns a JSON document with purchase metadata plus product lines. Rust deserialises it into `ProcessTicketResponse`.
- `routes::ocr::process_ticket` only logs the structured response and returns it to the caller; the database remains unchanged.
- The PostgreSQL schema already contains the required domain tables:
  - `compras` (sql/schema/schema.sql:153)
  - `tickets_pdf` (sql/schema/schema.sql:201)
  - `productos` (sql/schema/schema.sql:80)
  - `compras_productos` (sql/schema/schema.sql:237)
  - `historico_precios` maintained via trigger `trigger_registrar_precio_historico` (sql/schema/schema.sql:503)

## 3. Data Contract With the OCR Worker

| Field | Type | Required | Target usage | Transformations / Notes |
| --- | --- | --- | --- | --- |
| `ticket_id` | string | yes | Correlation id, logging, potential idempotency key | Keep as-is; consider persisting in new table for ingestion tracking (see open questions). |
| `raw_text` | string | yes | Optional storage for audit/debug | Store in DB only if we add a text snapshot table; otherwise keep in logs. |
| `numero_factura` | string | recommended (PK) | `compras.numero_factura` | Normalise (trim, uppercase); reject if missing because table requires PK. |
| `fecha_hora` | string (ISO) | preferred | `compras.fecha_hora` | Parse into `NaiveDateTime`. If absent use `fecha` (date) combined with default time (e.g. midday) while logging warning. |
| `total` | float | preferred | `compras.total` | Convert to `Decimal`/`BigDecimal` via `rust_decimal::Decimal`. Validation: non-negative, matches sum of product totals within tolerance. |
| `tienda` | string | optional | `compras.tienda` | Trim; max 255 chars. |
| `ubicacion` | string | optional | `compras.ubicacion` | Trim. |
| `metodo_pago` | string | optional | `compras.metodo_pago` | Map to enum subset (`TARJETA BANCARIA`, `EFECTIVO`, `BIZUM`, `TRANSFERENCIA`). |
| `numero_operacion` | string | optional | `compras.numero_operacion` | Trim. |
| `productos` | array of `TicketProduct` | yes (when total > 0) | `compras_productos` rows and dynamic insert/update of `productos` | See per-field mapping below. |
| `iva_desglose` | array | optional | Potential future analytics | No target table yet; keep in response or extend schema later. |

`TicketProduct` mapping:

| Field | Target | Notes |
| --- | --- | --- |
| `nombre` | `productos.nombre`, `compras_productos.producto_nombre` | Normalise (trim, uppercase?). Decide strategy to avoid duplicates (slugify or fuzzy match). |
| `cantidad` | `compras_productos.cantidad` | Validate > 0, map to `NUMERIC(10,3)`. |
| `unidad` | `productos.unidad` | Map to allowed set (`unidad`, `kg`, `g`, `l`, `ml`). |
| `precio_unitario` | `compras_productos.precio_unitario`, `productos.precio_actual` | Non-negative decimal with 2 places. |
| `precio_total` | `compras_productos.precio_total` | Validate vs unit price * cantidad - descuento (tolerance 0.01). |
| `descuento` | `compras_productos.descuento` | Optional, defaults to 0. |
| `iva_porcentaje` | `compras_productos.iva_porcentaje` | Normalise to 0/4/10/21 when possible. |
| `iva_importe` | `compras_productos.iva_importe` | Optional; compute when missing using porcentaje and base. |

## 4. Proposed Backend Changes (Rust)

### 4.1 API Layer

- Update `routes::ocr::process_ticket` to require authenticated user context (JWT) to obtain `usuario_email`. This may reuse upcoming auth middleware or temporarily extract from request payload until middleware exists.
- After calling the OCR worker, delegate to a new service method `ingest_ticket_data(state, user_email, request, response)`.
- Return a response that includes persistence status (e.g. `{"ingested": true, "numero_factura": "...", ...}`) for the frontend.

### 4.2 Service Layer

- New module `services::ticket_ingestion` responsible for orchestration:
  - Validate mandatory fields (`numero_factura`, `productos`, `total`).
  - Prepare a domain struct (e.g. `IngestedTicket`) with typed fields.
  - Decode the PDF (`pdf_b64`) and capture metadata (size, filename).
  - Open a DB transaction and call repository functions (see below).
  - Handle idempotency: check if `compras.numero_factura` already exists. Options: return existing record, update with new data, or abort with conflict (plan to return conflict for now, log manual resolution path).
  - Emit tracing spans/timing for worker processing vs persistence.

### 4.3 Database Access Layer

- Extend `backend/src/db` with dedicated modules (e.g. `purchases.rs`, `products.rs`, `tickets.rs`) and expose them in `db/mod.rs`.
- Functions required:
  1. `get_purchase(numero_factura) -> Option<Purchase>`
  2. `insert_purchase(...) -> Purchase`
  3. `upsert_product(...) -> Product` (update brand/unit if missing, maintain `precio_actual` with timestamp rule)
  4. `insert_purchase_products(purchase_id, items: &[PurchaseProductInsert])`
  5. `insert_ticket_pdf(numero_factura, bytes, filename, size_bytes)`
- Use SQLx `query!` / `query_as!` macros with offline preparation.
- Wrap inserts in a `PgPool::begin().await?` transaction so that failures roll back everything. Leverage trigger `trigger_registrar_precio_historico` to maintain `historico_precios` automatically.

### 4.4 Models & Schema DTOs

- Create domain models mirroring `compras` and `compras_productos` (e.g. `Purchase`, `PurchaseProduct`).
- Add DTOs for ingestion results if we need to respond with structured data to the frontend (`TicketIngestionResponse`, `IngestedProduct`).
- Introduce conversion helpers between worker response DTOs and domain models (normalisation functions for strings, decimals, enums).

### 4.5 Error Handling

- Define new error variants in `AppError` for:
  - `MissingInvoiceNumber`
  - `InvalidTotals`
  - `DuplicatePurchase`
  - `DatabaseIntegrity`
- Map SQLx constraint errors (unique violation, FK failure) to meaningful API responses (409 Conflict or 422 Unprocessable Entity).
- Log payload excerpts (without PDF) in error cases to aid debugging.

### 4.6 Observability

- Add tracing spans around:
  - Python processing latency.
  - Persistence transaction latency.
  - Individual DB operations (optional).
- Emit counters (via metrics crate in future) for ingested tickets, duplicates, validation failures.

### 4.7 Testing Strategy

- Unit tests for normalisation helpers (string trimming, IVA mapping, rounding).
- Integration tests (`backend/tests/integration/ticket_ingestion_tests.rs`):
  - Happy path: insert fixture JSON, assert rows created in `compras`, `compras_productos`, `productos`.
  - Duplicate invoice scenario.
  - Missing mandatory data scenario.
  - Product name normalisation effect.
- Use test DB or SQLx test transaction rollback pattern.
- Optionally mock Python layer by injecting a fake `ProcessTicketResponse` (bypassing PyO3) to keep tests deterministic.

## 5. Data Flow Sequence

1. **Frontend** uploads PDF (base64) with authenticated request.
2. **Route** validates payload, extracts user email from JWT, starts tracing span.
3. **Service** calls OCR worker (`services::ocr::process_ticket`).
4. **Ticket ingestion service**:
   - Validates worker response.
   - Builds `IngestedTicket`.
   - Begins transaction.
   - Inserts/updates `productos`.
   - Inserts `compras` row.
   - Inserts `compras_productos` rows.
   - Inserts `tickets_pdf` row (PDF binary) if configured.
   - Commits transaction.
5. **Response** returns JSON summarising stored data (invoice number, totals, inserted product count).
6. **Frontend** displays confirmation and triggers UI refresh (next task).

## 6. Open Questions / Decisions Needed

1. **User association**: confirm JWT middleware will be ready; otherwise we must extend request payload to include the email temporarily.
2. **Product normalisation**: decide rule set (simple trim/uppercase vs. dedup by synonyms). For MVP we can use case-insensitive match with trimmed strings.
3. **Ticket raw text storage**: do we persist `raw_text` for audit? Option: add `tickets_texto` table or JSON column. Currently out of scope unless requested.
4. **Idempotency**: should re-processing the same invoice update existing data? Proposed default: reject with 409 to avoid accidental overwrites.
5. **IVA breakdown**: no dedicated table yet; we may extend schema later if analytics require aggregated IVA data.

## 7. Deliverables Checklist

- [ ] `services::ticket_ingestion` module with orchestration logic.
- [ ] Extended `db` layer for purchases/products/tickets.
- [ ] Updated OCR route enforcing auth and calling ingestion.
- [ ] New DTOs/models for ingestion results.
- [ ] Error definitions and logging improvements.
- [ ] Tests covering ingestion workflow.
- [ ] Documentation update (`docs/BACKEND_TICKET_INGESTION_PLAN.md`) and README pointers.

## 8. Guidance for Frontend Follow-Up

When the backend work is complete, the frontend should:

1. Display the ingestion summary returned by the API (invoice number, product count, total).
2. Surface validation errors (e.g., duplicate invoice) with actionable messaging.
3. Refresh relevant dashboards by triggering data fetch after successful ingestion.
4. Optionally show a preview of parsed products so the user can confirm accuracy.

This document should be kept in sync with implementation details as the integration progresses.

