use serde::Deserialize;
use validator::Validate;

use crate::services::OcrProcessTicketRequest;

/// Payload recibido desde el frontend para procesar un ticket.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TicketProcessPayload {
    #[validate(length(min = 1, message = "ticket_id es requerido"))]
    pub ticket_id: String,
    #[validate(length(min = 1, message = "file_name es requerido"))]
    pub file_name: String,
    #[serde(alias = "pdf_b64")]
    #[validate(length(min = 1, message = "file_content_b64 es requerido"))]
    pub file_content_b64: String,
    /// Email del usuario (temporal hasta que se implemente autenticacion JWT)
    #[validate(email(message = "usuario_email debe ser un email valido"))]
    pub usuario_email: Option<String>,
}

impl From<TicketProcessPayload> for OcrProcessTicketRequest {
    fn from(value: TicketProcessPayload) -> Self {
        OcrProcessTicketRequest {
            ticket_id: value.ticket_id,
            file_name: value.file_name,
            file_content_b64: value.file_content_b64,
        }
    }
}
