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
    } = auth_user;

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

    tracing::info!(
        "Procesando ticket '{}' para el usuario {}",
        payload.file_name,
        authenticated_email
    );

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
        tracing::info!("Ingesta solicitada para el usuario {}", email);

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
                tracing::info!(
                    "Ticket {} ingestado correctamente",
                    ingestion.numero_factura
                );
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
    let raw_preview = ocr_result
        .raw_text
        .chars()
        .take(320)
        .collect::<String>();

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
    tracing::info!("OCR completado para ticket {}", result.ticket_id);

    if let Some(ref factura) = result.numero_factura {
        tracing::info!("  Numero de factura: {}", factura);
    }

    if let Some(ref fecha) = result.fecha {
        tracing::info!("  Fecha: {}", fecha);
    }

    if let Some(total) = result.total {
        tracing::info!("  Total detectado: {:.2}", total);
    }

    tracing::info!("  Productos detectados: {}", result.productos.len());

    if let Some(profile) = &result.processing_profile {
        tracing::info!("  Pipeline OCR: {}", profile);
    }

    if !result.productos.is_empty() {
        for (idx, prod) in result.productos.iter().enumerate() {
            tracing::info!(
                "    {}. {} - {} {} x {:.2} = {:.2}",
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
        tracing::info!("  IVA desglosado:");
        for iva in &result.iva_desglose {
            tracing::info!(
                "    {}% -> base {:.2}, cuota {:.2}",
                iva.porcentaje,
                iva.base_imponible,
                iva.cuota
            );
        }
    }

    if !result.warnings.is_empty() {
        for warning in &result.warnings {
            tracing::warn!("  Advertencia OCR: {}", warning);
        }
    }
}

fn map_intelligence_error(err: IntelligenceClientError) -> AppError {
    match err {
        IntelligenceClientError::Timeout | IntelligenceClientError::ServiceUnavailable => {
            AppError::ServiceUnavailable("Servicio de inteligencia no disponible".to_string())
        }
        IntelligenceClientError::UnexpectedStatus { status, body } => {
            if status == ReqStatusCode::BAD_REQUEST
                || status == ReqStatusCode::UNPROCESSABLE_ENTITY
                || status == ReqStatusCode::UNSUPPORTED_MEDIA_TYPE
            {
                AppError::BadRequest(body)
            } else {
                AppError::InternalError(format!(
                    "Fallo en el servicio de inteligencia ({}): {}",
                    status, body
                ))
            }
        }
        IntelligenceClientError::Deserialize(message) => {
            AppError::InternalError(format!("Respuesta OCR invalida: {}", message))
        }
        IntelligenceClientError::Request(err) => AppError::InternalError(format!(
            "No se pudo contactar con el servicio de inteligencia: {}",
            err
        )),
    }
}

/// Router para los endpoints relacionados con OCR.
pub fn ocr_router(state: AppState) -> Router {
    Router::new()
        .route("/process", post(process_ticket))
        .with_state(state)
}
