use axum::{
    extract::{DefaultBodyLimit, State},
    routing::post,
    Json, Router,
};
use base64::{engine::general_purpose, Engine as _};
use reqwest::StatusCode as ReqStatusCode;
use serde::Serialize;
use validator::Validate;

use super::auth::AppState;
use crate::{
    error::{AppError, AppResult},
    middleware::AuthenticatedUser,
    models::{detect_ticket_mime, photo_extension, TicketPdfInsert, MAX_TICKET_FILE_BYTES},
    schema::{TicketConfirmPayload, TicketProcessPayload},
    services::{
        ingest_ticket, photo_review::prepare_photo_products, IntelligenceClientError, IvaBreakdown,
        OcrProcessTicketResponse, TicketIngestionResponse, TicketProduct,
    },
};

/// Límite del cuerpo JSON en estas rutas: una foto de 10 MB ocupa ~13,4 MB en base64
/// (el límite por defecto de axum, 2 MB, rechazaba cualquier foto de móvil).
const OCR_BODY_LIMIT: usize = 16 * 1024 * 1024;

/// Respuesta combinada del procesamiento y la ingesta del ticket.
#[derive(Debug, Clone, Serialize)]
pub struct TicketProcessAndIngestResponse {
    /// Datos resumidos del OCR
    pub ocr: OcrResponseSummary,
    /// Resultado de la ingesta en base de datos (si se solicito)
    pub ingestion: Option<TicketIngestionResponse>,
}

/// Resumen del OCR devuelto al frontend: todo lo necesario para revisar y corregir el
/// ticket antes de guardarlo con `/confirm`.
#[derive(Debug, Clone, Serialize)]
pub struct OcrResponseSummary {
    pub ticket_id: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub fecha_hora: Option<String>,
    pub total: Option<f64>,
    pub productos_detectados: usize,
    pub productos: Vec<TicketProduct>,
    pub iva_desglose: Vec<IvaBreakdown>,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub metodo_pago: Option<String>,
    pub numero_operacion: Option<String>,
    pub processing_profile: Option<String>,
    pub warnings: Vec<String>,
    pub raw_text_preview: Option<String>,
}

impl From<&OcrProcessTicketResponse> for OcrResponseSummary {
    fn from(ocr: &OcrProcessTicketResponse) -> Self {
        Self {
            ticket_id: ocr.ticket_id.clone(),
            numero_factura: ocr.numero_factura.clone(),
            fecha: ocr.fecha.clone(),
            fecha_hora: ocr.fecha_hora.clone(),
            total: ocr.total,
            productos_detectados: ocr.productos.len(),
            productos: ocr.productos.clone(),
            iva_desglose: ocr.iva_desglose.clone(),
            tienda: ocr.tienda.clone(),
            ubicacion: ocr.ubicacion.clone(),
            metodo_pago: ocr.metodo_pago.clone(),
            numero_operacion: ocr.numero_operacion.clone(),
            processing_profile: ocr.processing_profile.clone(),
            warnings: ocr.warnings.clone(),
            raw_text_preview: Some(ocr.raw_text.chars().take(320).collect()),
        }
    }
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
                tracing::error!("Fallo al ingerir ticket");
                return Err(err);
            }
        }
    } else {
        tracing::info!("No se solicito ingesta (usuario_email ausente)");
        None
    };

    // 3. Construir respuesta
    Ok(Json(TicketProcessAndIngestResponse {
        ocr: OcrResponseSummary::from(&ocr_result),
        ingestion: ingestion_result,
    }))
}

/// Guarda la foto de un ticket ya leída y revisada por el usuario, sin repetir el OCR.
///
/// Solo para fotos: los PDF digitales se guardan por `/process` como siempre. Los datos
/// llegan del cliente, así que pasan las mismas validaciones que los del OCR (totales,
/// duplicados, coherencia de importes) y se guardan a nombre del usuario autenticado.
pub async fn confirm_ticket(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Json(payload): Json<TicketConfirmPayload>,
) -> AppResult<Json<TicketIngestionResponse>> {
    payload
        .validate()
        .map_err(|err| AppError::BadRequest(format!("Validacion fallida: {}", err)))?;

    if auth_user.is_demo {
        return Err(AppError::DemoUserRestriction);
    }

    let mime = validate_photo_file(&payload.file_content_b64)?;
    validate_reviewed_products(&payload.productos)?;
    if !payload.total.is_finite() {
        return Err(AppError::BadRequest("Total inválido".to_string()));
    }

    let TicketConfirmPayload {
        file_name,
        file_content_b64,
        numero_factura,
        fecha_hora,
        total,
        tienda,
        ubicacion,
        metodo_pago,
        numero_operacion,
        productos,
        iva_desglose,
    } = payload;

    // La tabla exige una extensión de imagen: una foto pegada o renombrada puede no traerla
    let file_name = if TicketPdfInsert::validate_photo_extension(&file_name) {
        file_name
    } else {
        format!("{}.{}", file_name.trim(), photo_extension(mime))
    };

    // Nombres del catálogo y una línea por producto; al unir líneas se recalcula el precio
    // unitario, así que se vuelve a comprobar lo que exigirá la base de datos.
    let productos = prepare_photo_products(&state.db_pool, productos).await?;
    validate_reviewed_products(&productos)?;

    let reviewed = OcrProcessTicketResponse {
        ticket_id: String::new(),
        raw_text: String::new(),
        numero_factura: Some(numero_factura),
        fecha: None,
        fecha_hora: Some(fecha_hora),
        total: Some(total),
        tienda,
        ubicacion,
        metodo_pago,
        numero_operacion,
        productos,
        iva_desglose,
        processing_profile: Some("revisado".to_string()),
        warnings: Vec::new(),
    };

    let ingestion = ingest_ticket(
        &state.db_pool,
        &auth_user.email,
        &file_content_b64,
        &file_name,
        reviewed,
    )
    .await?;

    tracing::info!(
        productos = ingestion.productos_insertados,
        "Ticket revisado guardado"
    );
    Ok(Json(ingestion))
}

/// Comprueba que el archivo sea una foto (por su contenido) de 10 MB como máximo y
/// devuelve su tipo MIME.
fn validate_photo_file(file_b64: &str) -> AppResult<&'static str> {
    // Sin decodificar: el base64 ocupa 4/3 del archivo
    if file_b64.len() > MAX_TICKET_FILE_BYTES.div_ceil(3) * 4 {
        return Err(AppError::BadRequest(
            "El archivo excede el tamaño máximo de 10MB".to_string(),
        ));
    }
    let bytes = general_purpose::STANDARD
        .decode(file_b64)
        .map_err(|_| AppError::BadRequest("El archivo no se ha podido leer".to_string()))?;
    match detect_ticket_mime(&bytes) {
        Some(mime) if mime.starts_with("image/") => Ok(mime),
        _ => Err(AppError::BadRequest(
            "Solo las fotos de tickets se guardan revisadas (JPG, PNG, WEBP o HEIC)".to_string(),
        )),
    }
}

/// Comprueba lo que la base de datos exigiría a cada línea, con mensajes que el usuario
/// pueda corregir en la revisión (la restricción `precio_coherente` daría un error genérico).
fn validate_reviewed_products(productos: &[TicketProduct]) -> AppResult<()> {
    fn cents(value: f64) -> f64 {
        (value * 100.0).round() / 100.0
    }

    for p in productos {
        let nombre = p.nombre.trim();
        if nombre.is_empty() || nombre.chars().count() > 255 {
            return Err(AppError::InvalidTicketData(
                "Hay un producto sin nombre o con un nombre demasiado largo".to_string(),
            ));
        }
        let numbers = [p.cantidad, p.precio_unitario, p.precio_total, p.descuento];
        if numbers.iter().any(|n| !n.is_finite() || *n < 0.0) || p.cantidad <= 0.0 {
            return Err(AppError::InvalidTicketData(format!(
                "«{nombre}»: la cantidad y los importes deben ser números positivos"
            )));
        }
        let cantidad = (p.cantidad * 1000.0).round() / 1000.0;
        let esperado = cents(p.precio_unitario) * cantidad - cents(p.descuento);
        if (cents(p.precio_total) - esperado).abs() >= 0.01 - 1e-9 {
            return Err(AppError::InvalidTicketData(format!(
                "«{nombre}»: el importe no cuadra con la cantidad por el precio unitario"
            )));
        }
    }
    Ok(())
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

/// Mensaje que el lector de fotos redacta para el usuario ("La imagen no parece un ticket
/// de Mercadona."...), marcado con `code: "ticket_rejected"`. Cualquier otro error del
/// servicio (los de PDF incluidos) mantiene el mensaje genérico de siempre.
fn photo_rejection_message(body: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(body).ok()?;
    if value.get("code")?.as_str()? != "ticket_rejected" {
        return None;
    }
    let detail = value.get("detail")?.as_str()?.trim();
    (!detail.is_empty() && detail.chars().count() <= 200).then(|| detail.to_string())
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
                AppError::BadRequest(
                    photo_rejection_message(&body)
                        .unwrap_or_else(|| "No se pudo procesar el ticket".to_string()),
                )
            } else if status == ReqStatusCode::SERVICE_UNAVAILABLE {
                AppError::ServiceUnavailable("Lector de tickets no disponible".to_string())
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
        .route("/confirm", post(confirm_ticket))
        .layer(DefaultBodyLimit::max(OCR_BODY_LIMIT))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(cantidad: f64, precio_unitario: f64, precio_total: f64) -> TicketProduct {
        TicketProduct {
            nombre: "LECHE".into(),
            cantidad,
            unidad: "unidad".into(),
            precio_unitario,
            precio_total,
            descuento: 0.0,
            iva_porcentaje: 4.0,
            iva_importe: 0.0,
        }
    }

    #[test]
    fn el_motivo_de_rechazo_de_una_foto_llega_al_usuario() {
        let err = map_intelligence_error(IntelligenceClientError::UnexpectedStatus {
            status: ReqStatusCode::UNPROCESSABLE_ENTITY,
            body: r#"{"detail":"La imagen no parece un ticket de Mercadona.","code":"ticket_rejected"}"#
                .into(),
        });
        assert!(
            matches!(err, AppError::BadRequest(ref m) if m == "La imagen no parece un ticket de Mercadona.")
        );

        // Los errores de PDF siguen dando el mensaje de siempre
        let err = map_intelligence_error(IntelligenceClientError::UnexpectedStatus {
            status: ReqStatusCode::UNPROCESSABLE_ENTITY,
            body: r#"{"detail":"No se pudo procesar el archivo"}"#.into(),
        });
        assert!(matches!(err, AppError::BadRequest(ref m) if m == "No se pudo procesar el ticket"));

        let err = map_intelligence_error(IntelligenceClientError::UnexpectedStatus {
            status: ReqStatusCode::SERVICE_UNAVAILABLE,
            body: String::new(),
        });
        assert!(matches!(err, AppError::ServiceUnavailable(_)));
    }

    #[test]
    fn la_confirmacion_revisada_solo_admite_fotos() {
        let jpeg = general_purpose::STANDARD.encode([0xFF, 0xD8, 0xFF, 0xE0, 0, 0]);
        assert_eq!(validate_photo_file(&jpeg).unwrap(), "image/jpeg");

        let pdf = general_purpose::STANDARD.encode(b"%PDF-1.4\n");
        assert!(
            matches!(validate_photo_file(&pdf), Err(AppError::BadRequest(m)) if m.contains("Solo las fotos"))
        );
        assert!(validate_photo_file(&general_purpose::STANDARD.encode(b"PK\x03\x04")).is_err());
        assert!(validate_photo_file("no es base64!").is_err());
    }

    #[test]
    fn exige_que_cada_importe_cuadre_con_cantidad_por_precio() {
        assert!(validate_reviewed_products(&[product(2.0, 0.95, 1.90)]).is_ok());
        // Producto a peso: 0,590 kg × 2,70 €/kg = 1,593 € -> 1,59 €
        assert!(validate_reviewed_products(&[product(0.59, 2.70, 1.59)]).is_ok());

        let err = validate_reviewed_products(&[product(2.0, 0.95, 2.90)]).unwrap_err();
        assert!(matches!(err, AppError::InvalidTicketData(m) if m.contains("no cuadra")));
        assert!(validate_reviewed_products(&[product(0.0, 1.0, 0.0)]).is_err());
        assert!(validate_reviewed_products(&[product(1.0, f64::NAN, 1.0)]).is_err());
        assert!(validate_reviewed_products(&[TicketProduct {
            nombre: "  ".into(),
            ..product(1.0, 1.0, 1.0)
        }])
        .is_err());
    }
}
