use axum::{extract::State, routing::post, Json, Router};
use validator::Validate;

use super::auth::AppState;
use crate::{
    error::{AppError, AppResult},
    schema::{TicketProcessPayload, TicketProcessResponse},
    services::{process_ticket_ocr, OcrError},
};

/// Handler que procesa tickets PDF utilizando la lógica Python embebida.
pub async fn process_ticket(
    State(_state): State<AppState>,
    Json(payload): Json<TicketProcessPayload>,
) -> AppResult<Json<TicketProcessResponse>> {
    payload
        .validate()
        .map_err(|err| AppError::BadRequest(format!("Validación fallida: {}", err)))?;

    tracing::info!("📄 Procesando ticket: {}", payload.file_name);

    let request = payload.into();
    let result = process_ticket_ocr(request).await.map_err(map_ocr_error)?;

    // Logging detallado del resultado
    tracing::info!("✅ Ticket procesado exitosamente:");
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

    tracing::info!("─────────────────────────────────────────");

    Ok(Json(result))
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
