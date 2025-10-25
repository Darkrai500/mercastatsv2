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

    let request = payload.into();
    let result = process_ticket_ocr(request).await.map_err(map_ocr_error)?;

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
