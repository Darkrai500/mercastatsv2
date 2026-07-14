use axum::{extract::State, routing::post, Json, Router};
use reqwest::StatusCode as ReqStatusCode;
use serde::Serialize;
use validator::Validate;

use super::auth::AppState;
use crate::{
    error::{AppError, AppResult},
    middleware::AuthenticatedUser,
    schema::TicketProcessPayload,
    services::{
        ingest_ticket, IntelligenceClientError, OcrProcessTicketResponse, TicketIngestionResponse,
        TicketProduct,
    },
};

/// Respuesta combinada del procesamiento y la ingesta del ticket.
#[derive(Debug, Clone, Serialize)]
pub struct TicketProcessAndIngestResponse {
    /// Datos resumidos del OCR
    pub ocr: OcrResponseSummary,
    /// Resultado de la ingesta en base de datos (si se solicito)
    pub ingestion: Option<TicketIngestionResponse>,
}

/// Resumen del OCR devuelto al frontend.
#[derive(Debug, Clone, Serialize)]
pub struct OcrResponseSummary {
    pub ticket_id: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub total: Option<f64>,
    pub productos_detectados: usize,
    #[serde(default)]
    pub productos: Vec<TicketProduct>,
    #[serde(default)]
    pub tienda: Option<String>,
    #[serde(default)]
    pub processing_profile: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub raw_text_preview: Option<String>,
}

/// Procesa un ticket y, si se solicita, lo ingesta en la base de datos.
pub async fn process_ticket(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<TicketProcessPayload>,
) -> AppResult<Json<TicketProcessAndIngestResponse>> {
    payload
        .validate()
        .map_err(|err| AppError::BadRequest(format!("Validacion fallida: {}", err)))?;

    let AuthenticatedUser {
        email: authenticated_email,
        is_demo,
    } = auth_user;

    if is_demo {
        return Err(AppError::DemoUserRestriction);
    }

    if let Some(ref mime) = payload.mime_type {
        let allowed = mime.starts_with("image/") || mime == "application/pdf";
        if !allowed {
            return Err(AppError::BadRequest(format!(
                "Formato no soportado: {}. Solo se aceptan PDF o imagen.",
                mime
            )));
        }
    }

    if let Some(ref email) = payload.usuario_email {
        if email != &authenticated_email {
            return Err(AppError::Unauthorized(
                "No puedes procesar tickets de otro usuario".to_string(),
            ));
        }
    }

    tracing::info!("Procesando ticket autenticado");

    // Clonar datos necesarios antes de consumir el payload
    let file_content_b64 = payload.file_content_b64.clone();
    let file_name = payload.file_name.clone();
    let usuario_email = payload.usuario_email.clone();

    // 1. Ejecutar OCR
    let request = payload.into();
    let ocr_result = state
        .intelligence_client
        .process_ticket(request)
        .await
        .map_err(map_intelligence_error)?;
    log_ocr_result(&ocr_result);

    // 2. Ingestar ticket si el usuario lo solicito
    let ingestion_result = if let Some(email) = usuario_email {
        tracing::info!("Ingesta de ticket solicitada");

        match ingest_ticket(
            &state.db_pool,
            &email,
            &file_content_b64,
            &file_name,
            ocr_result.clone(),
        )
        .await
        {
            Ok(ingestion) => {
                tracing::info!("Ticket ingestado correctamente");
                Some(ingestion)
            }
            Err(err) => {
                tracing::error!("Fallo al ingerir ticket: {:?}", err);
                return Err(err);
            }
        }
    } else {
        tracing::info!("No se solicito ingesta (usuario_email ausente)");
        None
    };

    // 3. Construir respuesta
    let raw_preview = ocr_result.raw_text.chars().take(320).collect::<String>();

    let response = TicketProcessAndIngestResponse {
        ocr: OcrResponseSummary {
            ticket_id: ocr_result.ticket_id.clone(),
            numero_factura: ocr_result.numero_factura.clone(),
            fecha: ocr_result.fecha.clone(),
            total: ocr_result.total,
            productos_detectados: ocr_result.productos.len(),
            productos: ocr_result.productos.clone(),
            tienda: ocr_result.tienda.clone(),
            processing_profile: ocr_result.processing_profile.clone(),
            warnings: ocr_result.warnings.clone(),
            raw_text_preview: Some(raw_preview),
        },
        ingestion: ingestion_result,
    };

    Ok(Json(response))
}

/// Logging estructurado del resultado del OCR para facilitar depuracion.
fn log_ocr_result(result: &OcrProcessTicketResponse) {
    tracing::info!(
        productos_detectados = result.productos.len(),
        avisos = result.warnings.len(),
        pipeline = result
            .processing_profile
            .as_deref()
            .unwrap_or("desconocido"),
        "OCR completado"
    );
}

fn map_intelligence_error(err: IntelligenceClientError) -> AppError {
    match err {
        IntelligenceClientError::Timeout | IntelligenceClientError::ServiceUnavailable => {
            AppError::ServiceUnavailable("Servicio de inteligencia no disponible".to_string())
        }
        IntelligenceClientError::UnexpectedStatus { status, .. } => {
            if status == ReqStatusCode::BAD_REQUEST
                || status == ReqStatusCode::UNPROCESSABLE_ENTITY
                || status == ReqStatusCode::UNSUPPORTED_MEDIA_TYPE
            {
                AppError::BadRequest("No se pudo procesar el ticket".to_string())
            } else {
                AppError::InternalError(format!(
                    "Fallo en el servicio de inteligencia ({})",
                    status
                ))
            }
        }
        IntelligenceClientError::Deserialize(_) => {
            AppError::InternalError("Respuesta OCR invalida".to_string())
        }
        IntelligenceClientError::Request(_) => AppError::InternalError(
            "No se pudo contactar con el servicio de inteligencia".to_string(),
        ),
    }
}

/// Router para los endpoints relacionados con OCR.
pub fn ocr_router(state: AppState) -> Router {
    Router::new()
        .route("/process", post(process_ticket))
        .with_state(state)
}
