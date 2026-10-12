//! Sincronización automática: busca en Gmail los correos con ticket, descarga el PDF y lo pasa
//! por el mismo OCR e ingesta que la subida manual.
//!
//! Política de privacidad del repositorio: aquí no se registra nunca un email, un asunto, un
//! identificador de mensaje, una factura ni un importe; solo recuentos y códigos de error.

use std::time::Duration;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use reqwest::StatusCode;

use crate::{
    db,
    error::AppError,
    models::google_account::ESTADO_ACTIVA,
    models::{MessageOutcome, SyncCounts},
    routes::auth::AppState,
    services::{
        gmail::{extract_pdf_attachments, sender_address, GmailError, PdfAttachment, PdfSource},
        google::GoogleError,
        google_integration::{GoogleIntegration, SyncPermit},
        ingest_ticket, IntelligenceClientError, OcrProcessTicketRequest,
    },
};

/// Veces que se reintenta un mensaje cuyo contenido no se pudo procesar.
pub const MAX_ATTEMPTS: i32 = 3;
/// Tope de mensajes nuevos por ejecución (un primer import enorme se completa en varias).
const MAX_MESSAGES_PER_RUN: usize = 300;
/// Mismo límite que la tabla `tickets_pdf`.
const MAX_PDF_BYTES: usize = 10 * 1024 * 1024;

// Códigos que se guardan en `cuentas_google.ultima_sync_error` y ve el frontend.
pub const ERR_TOKEN_REVOCADO: &str = "token_revocado";
pub const ERR_CLAVE_CIFRADO: &str = "clave_cifrado_invalida";
pub const ERR_GMAIL_DENEGADO: &str = "gmail_denegado";
pub const ERR_GOOGLE_NO_DISPONIBLE: &str = "google_no_disponible";
pub const ERR_GOOGLE_RECHAZADO: &str = "google_rechazado";
pub const ERR_OCR_NO_DISPONIBLE: &str = "ocr_no_disponible";
pub const ERR_INTERNO: &str = "error_interno";

#[derive(Debug, PartialEq, Eq)]
pub enum SyncError {
    NotConfigured,
    NotConnected,
    /// El token se revocó o caducó: hay que volver a conectar la cuenta
    NeedsReconnect,
    AlreadyRunning,
    Failed(&'static str),
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct SyncReport {
    pub counts: SyncCounts,
    /// Correos sin PDF (o de otro remitente), ignorados
    pub sin_pdf: u32,
    /// Correos nuevos que quedaron para la próxima ejecución (por el tope por ejecución)
    pub pendientes: u32,
}

enum MessageError {
    /// El mensaje desapareció entre el listado y la lectura: se ignora
    Skip,
    /// Algo transitorio o de configuración: se corta la ejecución y se reintenta más tarde
    Abort(&'static str),
}

/// Sincroniza la bandeja de un usuario. Seguro de llamar a la vez desde el planificador y desde
/// "Sincronizar ahora": la segunda llamada recibe `AlreadyRunning`.
pub async fn sync_user(state: &AppState, email: &str) -> Result<SyncReport, SyncError> {
    let google = state.google.as_ref().ok_or(SyncError::NotConfigured)?;
    let permit = google
        .try_begin_sync(email)
        .ok_or(SyncError::AlreadyRunning)?;
    sync_with_permit(state, email, permit).await
}

/// Reserva la sincronización AHORA (así el estado ya figura como "sincronizando" en cuanto se
/// responde al usuario) y la ejecuta en segundo plano.
pub fn spawn_sync(state: &AppState, email: &str) -> Result<(), SyncError> {
    let google = state.google.as_ref().ok_or(SyncError::NotConfigured)?;
    let permit = google
        .try_begin_sync(email)
        .ok_or(SyncError::AlreadyRunning)?;
    let state = state.clone();
    let email = email.to_string();
    tokio::spawn(async move {
        let _ = sync_with_permit(&state, &email, permit).await;
    });
    Ok(())
}

async fn sync_with_permit(
    state: &AppState,
    email: &str,
    _permit: SyncPermit,
) -> Result<SyncReport, SyncError> {
    let google = state.google.as_ref().ok_or(SyncError::NotConfigured)?;
    let pool = &state.db_pool;
    let account = db::google_accounts::find_by_user(pool, email)
        .await
        .map_err(|_| SyncError::Failed(ERR_INTERNO))?
        .ok_or(SyncError::NotConnected)?;
    if account.estado != ESTADO_ACTIVA {
        return Err(SyncError::NeedsReconnect);
    }

    db::google_accounts::mark_sync_started(pool, email)
        .await
        .map_err(|_| SyncError::Failed(ERR_INTERNO))?;

    let mut report = SyncReport::default();
    let outcome = run_sync(state, google, &account, &mut report).await;

    let error_code = outcome.as_ref().err().copied();
    if db::google_accounts::mark_sync_finished(pool, email, report.counts, error_code)
        .await
        .is_err()
    {
        tracing::error!("No se pudo guardar el resultado de la sincronización de Gmail");
    }

    tracing::info!(
        importados = report.counts.importados,
        duplicados = report.counts.duplicados,
        fallidos = report.counts.fallidos,
        sin_pdf = report.sin_pdf,
        pendientes = report.pendientes,
        error = error_code.unwrap_or("ninguno"),
        "Sincronización de Gmail terminada"
    );

    match outcome {
        Ok(()) => Ok(report),
        Err(ERR_TOKEN_REVOCADO) | Err(ERR_CLAVE_CIFRADO) => Err(SyncError::NeedsReconnect),
        Err(code) => Err(SyncError::Failed(code)),
    }
}

async fn run_sync(
    state: &AppState,
    google: &GoogleIntegration,
    account: &crate::models::GoogleAccount,
    report: &mut SyncReport,
) -> Result<(), &'static str> {
    let pool = &state.db_pool;
    let email = account.usuario_email.as_str();

    // 1. Access token a partir del refresh token cifrado
    let refresh_token = match google
        .cipher
        .decrypt(&account.refresh_token_cifrado, email)
        .ok()
        .and_then(|bytes| String::from_utf8(bytes).ok())
    {
        Some(token) => token,
        None => {
            // Clave de cifrado cambiada o fila manipulada: el token guardado no sirve
            let _ = db::google_accounts::mark_needs_reconnect(pool, email).await;
            return Err(ERR_CLAVE_CIFRADO);
        }
    };

    let access_token = match google.oauth.refresh_access_token(&refresh_token).await {
        Ok(token) => token,
        Err(GoogleError::InvalidGrant) => {
            let _ = db::google_accounts::mark_needs_reconnect(pool, email).await;
            return Err(ERR_TOKEN_REVOCADO);
        }
        Err(GoogleError::Rejected(_)) => return Err(ERR_GOOGLE_RECHAZADO),
        Err(GoogleError::Unavailable) | Err(GoogleError::BadResponse) => {
            return Err(ERR_GOOGLE_NO_DISPONIBLE)
        }
    };

    // 2. Qué mensajes son nuevos
    let ids = google
        .gmail
        .list_message_ids(&access_token, &google.search_query())
        .await
        .map_err(gmail_error_code)?;
    let processed = db::google_accounts::processed_messages(pool, email)
        .await
        .map_err(|_| ERR_INTERNO)?;

    let pending: Vec<&String> = ids
        .iter()
        .filter(|id| match processed.get(*id) {
            None => true,
            Some(p) => p.resultado == MessageOutcome::Fallido.as_str() && p.intentos < MAX_ATTEMPTS,
        })
        .collect();
    report.pendientes = pending.len() as u32;

    // 3. Procesarlos (del más reciente al más antiguo)
    for message_id in pending.into_iter().take(MAX_MESSAGES_PER_RUN) {
        match process_message(state, google, &access_token, email, message_id).await {
            Ok(outcome) => {
                match outcome {
                    MessageOutcome::Importado => report.counts.importados += 1,
                    MessageOutcome::Duplicado => report.counts.duplicados += 1,
                    MessageOutcome::Fallido => report.counts.fallidos += 1,
                    MessageOutcome::SinPdf => report.sin_pdf += 1,
                }
                db::google_accounts::record_message(pool, email, message_id, outcome)
                    .await
                    .map_err(|_| ERR_INTERNO)?;
            }
            Err(MessageError::Skip) => {}
            Err(MessageError::Abort(code)) => return Err(code),
        }
        report.pendientes -= 1;
    }
    Ok(())
}

async fn process_message(
    state: &AppState,
    google: &GoogleIntegration,
    access_token: &str,
    email: &str,
    message_id: &str,
) -> Result<MessageOutcome, MessageError> {
    let message = match google.gmail.get_message(access_token, message_id).await {
        Ok(message) => message,
        Err(GmailError::NotFound) => return Err(MessageError::Skip),
        Err(err) => return Err(MessageError::Abort(gmail_error_code(err))),
    };
    let Some(payload) = message.payload else {
        return Ok(MessageOutcome::SinPdf);
    };

    // La búsqueda `from:` de Gmail también casa con el nombre visible: se exige la dirección real.
    if sender_address(&payload).as_deref() != Some(google.sender.as_str()) {
        return Ok(MessageOutcome::SinPdf);
    }

    let pdfs = extract_pdf_attachments(&payload);
    if pdfs.is_empty() {
        return Ok(MessageOutcome::SinPdf);
    }

    let mut outcomes = Vec::with_capacity(pdfs.len());
    for (index, pdf) in pdfs.iter().enumerate() {
        outcomes
            .push(process_pdf(state, google, access_token, email, message_id, index, pdf).await?);
    }
    Ok(merge_outcomes(&outcomes))
}

/// Resultado de un mensaje con varios PDF: si algo se importó, importado; si algo falló, fallido
/// (para reintentarlo); si todo eran duplicados, duplicado.
fn merge_outcomes(outcomes: &[MessageOutcome]) -> MessageOutcome {
    let has = |wanted| outcomes.contains(&wanted);
    if has(MessageOutcome::Importado) {
        MessageOutcome::Importado
    } else if has(MessageOutcome::Fallido) {
        MessageOutcome::Fallido
    } else if has(MessageOutcome::Duplicado) {
        MessageOutcome::Duplicado
    } else {
        MessageOutcome::SinPdf
    }
}

async fn process_pdf(
    state: &AppState,
    google: &GoogleIntegration,
    access_token: &str,
    email: &str,
    message_id: &str,
    index: usize,
    pdf: &PdfAttachment,
) -> Result<MessageOutcome, MessageError> {
    if pdf.size.is_some_and(|size| size as usize > MAX_PDF_BYTES) {
        return Ok(MessageOutcome::Fallido);
    }

    let bytes = match &pdf.source {
        PdfSource::Inline(bytes) => bytes.clone(),
        PdfSource::Attachment(attachment_id) => {
            match google
                .gmail
                .get_attachment(access_token, message_id, attachment_id)
                .await
            {
                Ok(bytes) => bytes,
                Err(GmailError::NotFound) | Err(GmailError::BadResponse) => {
                    return Ok(MessageOutcome::Fallido)
                }
                Err(err) => return Err(MessageError::Abort(gmail_error_code(err))),
            }
        }
    };

    // No se confía en el nombre ni en el MIME: el contenido tiene que ser un PDF de verdad.
    if bytes.len() > MAX_PDF_BYTES || !bytes.starts_with(b"%PDF-") {
        return Ok(MessageOutcome::Fallido);
    }

    let file_name = safe_pdf_name(&pdf.filename);
    let pdf_b64 = STANDARD.encode(&bytes);
    let request = OcrProcessTicketRequest {
        ticket_id: format!("gmail_{message_id}_{index}"),
        file_name: file_name.clone(),
        file_content_b64: pdf_b64.clone(),
        mime_type: Some("application/pdf".to_string()),
    };

    let ocr = match state.intelligence_client.process_ticket(request).await {
        Ok(ocr) => ocr,
        Err(err) => return classify_ocr_error(&err).map_err(MessageError::Abort),
    };

    match ingest_ticket(&state.db_pool, email, &pdf_b64, &file_name, ocr).await {
        Ok(_) => Ok(MessageOutcome::Importado),
        Err(err) => classify_ingest_error(&err).map_err(MessageError::Abort),
    }
}

/// Un fallo del OCR o es del PDF (no se reintentará hoy) o del servicio (se reintenta luego).
fn classify_ocr_error(err: &IntelligenceClientError) -> Result<MessageOutcome, &'static str> {
    match err {
        IntelligenceClientError::UnexpectedStatus { status, .. }
            if matches!(
                *status,
                StatusCode::BAD_REQUEST
                    | StatusCode::UNPROCESSABLE_ENTITY
                    | StatusCode::UNSUPPORTED_MEDIA_TYPE
                    | StatusCode::PAYLOAD_TOO_LARGE
            ) =>
        {
            Ok(MessageOutcome::Fallido)
        }
        IntelligenceClientError::Deserialize(_) => Ok(MessageOutcome::Fallido),
        _ => Err(ERR_OCR_NO_DISPONIBLE),
    }
}

/// Un ticket repetido no es un fallo; los datos inválidos son del ticket; el resto (BD caída…)
/// es transitorio y corta la ejecución.
fn classify_ingest_error(err: &AppError) -> Result<MessageOutcome, &'static str> {
    match err {
        AppError::DuplicatePurchase(_) => Ok(MessageOutcome::Duplicado),
        AppError::MissingInvoiceNumber
        | AppError::InvalidTotals(_)
        | AppError::InvalidTicketData(_)
        | AppError::DatabaseIntegrity(_)
        | AppError::BadRequest(_) => Ok(MessageOutcome::Fallido),
        _ => Err(ERR_INTERNO),
    }
}

fn gmail_error_code(err: GmailError) -> &'static str {
    match err {
        GmailError::Unauthorized | GmailError::Forbidden => ERR_GMAIL_DENEGADO,
        GmailError::NotFound | GmailError::Unavailable | GmailError::BadResponse => {
            ERR_GOOGLE_NO_DISPONIBLE
        }
    }
}

/// Nombre de archivo apto para `tickets_pdf` (termina en .pdf, sin rutas, ≤ 200 caracteres).
fn safe_pdf_name(original: &str) -> String {
    let base: String = original
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(200)
        .collect();
    let base = base.trim();
    if base.to_ascii_lowercase().ends_with(".pdf") && base.len() > 4 {
        base.to_string()
    } else if base.is_empty() {
        "ticket.pdf".to_string()
    } else {
        format!("{base}.pdf")
    }
}

/// Sincroniza todas las cuentas activas, una tras otra. Devuelve (cuentas, tickets importados).
pub async fn sync_all(state: &AppState) -> (usize, u32) {
    let emails = match db::google_accounts::list_active_emails(&state.db_pool).await {
        Ok(emails) => emails,
        Err(_) => {
            tracing::error!("No se pudo listar las cuentas de Gmail a sincronizar");
            return (0, 0);
        }
    };

    let mut importados = 0;
    for email in &emails {
        if let Ok(report) = sync_user(state, email).await {
            importados += report.counts.importados;
        }
    }
    (emails.len(), importados)
}

/// Tarea de fondo: sincroniza todas las cuentas cada `interval`.
pub async fn run_scheduler(state: AppState, interval: Duration) {
    // Margen tras el arranque para no competir con la compilación/inicio del resto de servicios
    tokio::time::sleep(Duration::from_secs(30)).await;
    loop {
        let (cuentas, importados) = sync_all(&state).await;
        if cuentas > 0 {
            tracing::info!(
                cuentas,
                importados,
                "Ronda de sincronización de Gmail completada"
            );
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nombres_de_archivo_seguros() {
        assert_eq!(safe_pdf_name("ticket.pdf"), "ticket.pdf");
        assert_eq!(safe_pdf_name("Ticket.PDF"), "Ticket.PDF");
        assert_eq!(safe_pdf_name("../../etc/passwd"), "passwd.pdf");
        assert_eq!(safe_pdf_name("C:\\tmp\\factura.pdf"), "factura.pdf");
        assert_eq!(safe_pdf_name(""), "ticket.pdf");
        assert_eq!(safe_pdf_name("   "), "ticket.pdf");
        assert_eq!(safe_pdf_name(".pdf"), ".pdf.pdf");
        assert_eq!(safe_pdf_name("ticket\n.pdf"), "ticket.pdf");
        assert!(safe_pdf_name(&"a".repeat(500)).len() <= 204);
        assert!(safe_pdf_name(&"a".repeat(500))
            .to_lowercase()
            .ends_with(".pdf"));
    }

    #[test]
    fn un_mensaje_con_varios_pdf_se_resume_bien() {
        use MessageOutcome::*;
        assert_eq!(merge_outcomes(&[Duplicado, Importado]), Importado);
        assert_eq!(merge_outcomes(&[Duplicado, Fallido]), Fallido);
        assert_eq!(merge_outcomes(&[Duplicado, Duplicado]), Duplicado);
        assert_eq!(merge_outcomes(&[]), SinPdf);
    }

    #[test]
    fn los_errores_del_ocr_se_clasifican() {
        let fallo = |status| IntelligenceClientError::UnexpectedStatus {
            status,
            body: String::new(),
        };
        // El PDF es el problema -> fallido (se reintenta unas veces)
        assert_eq!(
            classify_ocr_error(&fallo(StatusCode::UNPROCESSABLE_ENTITY)),
            Ok(MessageOutcome::Fallido)
        );
        // El servicio es el problema -> se corta la ejecución, sin marcar el mensaje
        assert_eq!(
            classify_ocr_error(&fallo(StatusCode::INTERNAL_SERVER_ERROR)),
            Err(ERR_OCR_NO_DISPONIBLE)
        );
        assert_eq!(
            classify_ocr_error(&fallo(StatusCode::UNAUTHORIZED)),
            Err(ERR_OCR_NO_DISPONIBLE)
        );
        assert_eq!(
            classify_ocr_error(&IntelligenceClientError::Timeout),
            Err(ERR_OCR_NO_DISPONIBLE)
        );
    }

    #[test]
    fn los_errores_de_ingesta_se_clasifican() {
        assert_eq!(
            classify_ingest_error(&AppError::DuplicatePurchase("x".into())),
            Ok(MessageOutcome::Duplicado)
        );
        assert_eq!(
            classify_ingest_error(&AppError::MissingInvoiceNumber),
            Ok(MessageOutcome::Fallido)
        );
        assert_eq!(
            classify_ingest_error(&AppError::InvalidTotals("x".into())),
            Ok(MessageOutcome::Fallido)
        );
        assert_eq!(
            classify_ingest_error(&AppError::DatabaseError("x".into())),
            Err(ERR_INTERNO)
        );
    }

    #[test]
    fn los_errores_de_gmail_se_traducen_a_codigos() {
        assert_eq!(gmail_error_code(GmailError::Forbidden), ERR_GMAIL_DENEGADO);
        assert_eq!(
            gmail_error_code(GmailError::Unauthorized),
            ERR_GMAIL_DENEGADO
        );
        assert_eq!(
            gmail_error_code(GmailError::Unavailable),
            ERR_GOOGLE_NO_DISPONIBLE
        );
    }
}
