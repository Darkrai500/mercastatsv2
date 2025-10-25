use serde::Deserialize;
use validator::Validate;

use crate::services::OcrProcessTicketRequest;
pub use crate::services::OcrProcessTicketResponse as TicketProcessResponse;

/// Payload recibido desde el frontend para procesar un ticket.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TicketProcessPayload {
    #[validate(length(min = 1, message = "ticket_id es requerido"))]
    pub ticket_id: String,
    #[validate(length(min = 1, message = "file_name es requerido"))]
    pub file_name: String,
    #[validate(length(min = 1, message = "pdf_b64 es requerido"))]
    pub pdf_b64: String,
}

impl From<TicketProcessPayload> for OcrProcessTicketRequest {
    fn from(value: TicketProcessPayload) -> Self {
        OcrProcessTicketRequest {
            ticket_id: value.ticket_id,
            file_name: value.file_name,
            pdf_b64: value.pdf_b64,
        }
    }
}
