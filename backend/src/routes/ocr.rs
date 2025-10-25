use axum::{extract::State, routing::post, Json, Router};
use serde::Serialize;
use validator::Validate;

use super::auth::AppState;
use crate::{
    error::{AppError, AppResult},
    schema::TicketProcessPayload,
    services::{ingest_ticket, process_ticket_ocr, OcrError, TicketIngestionResponse},
};

/// Respuesta extendida del procesamiento y la ingesta
#[derive(Debug, Clone, Serialize)]
pub struct TicketProcessAndIngestResponse {
    /// Información del OCR
    pub ocr: OcrResponseSummary,
    /// Información de la ingesta (None si usuario_email no se proporcionó)
    pub ingestion: Option<TicketIngestionResponse>,
}

/// Resumen de la respuesta OCR para el cliente
#[derive(Debug, Clone, Serialize)]
pub struct OcrResponseSummary {
    pub ticket_id: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub total: Option<f64>,
    pub productos_detectados: usize,
}

/// Handler que procesa tickets PDF utilizando la lógica Python embebida
/// y opcionalmente los ingesta en la base de datos.
pub async fn process_ticket(
    State(state): State<AppState>,
    Json(payload): Json<TicketProcessPayload>,
) -> AppResult<Json<TicketProcessAndIngestResponse>> {
    payload
        .validate()
        .map_err(|err| AppError::BadRequest(format!("Validación fallida: {}", err)))?;

    tracing::info!("📄 Procesando ticket: {}", payload.file_name);

    // Guardar datos necesarios antes de consumir el payload
    let pdf_b64 = payload.pdf_b64.clone();
    let file_name = payload.file_name.clone();
    let usuario_email = payload.usuario_email.clone();

    // 1. Procesar con OCR
    let request = payload.into();
    let ocr_result = process_ticket_ocr(request)
        .await
        .map_err(map_ocr_error)?;

    // Logging del OCR
    log_ocr_result(&ocr_result);

    // 2. Ingestar en la base de datos si se proporcionó usuario_email
    let ingestion_result = if let Some(email) = usuario_email {
        tracing::info!("💾 Ingesta solicitada para usuario: {}", email);

        match ingest_ticket(
            &state.db_pool,
            &email,
            &pdf_b64,
            &file_name,
            ocr_result.clone(),
        )
        .await
        {
            Ok(ingestion) => {
                tracing::info!(
                    "✅ Ticket {} ingestado exitosamente",
                    ingestion.numero_factura
                );
                Some(ingestion)
            }
            Err(e) => {
                // Loguear el error pero no fallar la request completa
                tracing::error!("❌ Error en la ingesta: {:?}", e);
                return Err(e);
            }
        }
    } else {
        tracing::info!("ℹ️  No se solicitó ingesta (usuario_email no proporcionado)");
        None
    };

    // 3. Construir respuesta
    let response = TicketProcessAndIngestResponse {
        ocr: OcrResponseSummary {
            ticket_id: ocr_result.ticket_id.clone(),
            numero_factura: ocr_result.numero_factura.clone(),
            fecha: ocr_result.fecha.clone(),
            total: ocr_result.total,
            productos_detectados: ocr_result.productos.len(),
        },
        ingestion: ingestion_result,
    };

    tracing::info!("─────────────────────────────────────────");

    Ok(Json(response))
}

/// Logging detallado del resultado del OCR
fn log_ocr_result(result: &crate::services::OcrProcessTicketResponse) {
    tracing::info!("✅ OCR completado exitosamente:");
    tracing::info!("   📋 ID: {}", result.ticket_id);

    if let Some(ref factura) = result.numero_factura {
        tracing::info!("   🧾 Factura: {}", factura);
    }

    if let Some(ref fecha) = result.fecha {
        tracing::info!("   📅 Fecha: {}", fecha);
    }

    if let Some(total) = result.total {
        tracing::info!("   💰 Total: {:.2}€", total);
    }

    if let Some(ref tienda) = result.tienda {
        tracing::info!("   🏪 Tienda: {}", tienda);
        if let Some(ref ubicacion) = result.ubicacion {
            tracing::info!("      📍 {}", ubicacion);
        }
    }

    if let Some(ref metodo) = result.metodo_pago {
        tracing::info!("   💳 Pago: {}", metodo);
    }

    tracing::info!("   🛒 Productos encontrados: {}", result.productos.len());

    if !result.productos.is_empty() {
        tracing::info!("   Lista de productos:");
        for (idx, prod) in result.productos.iter().enumerate() {
            tracing::info!(
                "      {}. {} - {} {} × {:.2}€ = {:.2}€",
                idx + 1,
                prod.nombre,
                prod.cantidad,
                prod.unidad,
                prod.precio_unitario,
                prod.precio_total
            );
        }
    }

    if !result.iva_desglose.is_empty() {
        tracing::info!("   📊 Desglose IVA:");
        for iva in &result.iva_desglose {
            tracing::info!(
                "      {}%: Base {:.2}€ → Cuota {:.2}€",
                iva.porcentaje,
                iva.base_imponible,
                iva.cuota
            );
        }
    }
}

fn map_ocr_error(err: OcrError) -> AppError {
    match err {
        OcrError::Parsing(message) => AppError::BadRequest(message),
        OcrError::Python(message) => {
            tracing::error!("Error Python durante OCR: {}", message);
            AppError::InternalError("Fallo en el motor OCR".to_string())
        }
        OcrError::Deserialize(error) => {
            tracing::error!("Error de deserialización OCR: {}", error);
            AppError::InternalError("Respuesta OCR inválida".to_string())
        }
        OcrError::Join(error) => {
            tracing::error!("Error al ejecutar OCR en hilo bloqueante: {}", error);
            AppError::InternalError("Error interno del servidor".to_string())
        }
    }
}

/// Router para los endpoints relacionados con OCR.
pub fn ocr_router(state: AppState) -> Router {
    Router::new()
        .route("/process", post(process_ticket))
        .with_state(state)
}
