use serde::Deserialize;
use validator::Validate;

use crate::services::{IvaBreakdown, OcrProcessTicketRequest, TicketProduct};

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
    /// MIME enviado por el cliente (opcional, el worker valida igualmente)
    #[validate(length(min = 1, message = "mime_type no puede estar vacio"))]
    pub mime_type: Option<String>,
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
            mime_type: value.mime_type,
        }
    }
}

/// Ticket revisado (y quizá corregido) por el usuario, listo para guardar sin repetir el OCR.
#[derive(Debug, Clone, Deserialize, Validate)]
pub struct TicketConfirmPayload {
    #[validate(length(min = 1, max = 255, message = "file_name es requerido"))]
    pub file_name: String,
    #[serde(alias = "pdf_b64")]
    #[validate(length(min = 1, message = "file_content_b64 es requerido"))]
    pub file_content_b64: String,
    #[validate(length(min = 1, max = 50, message = "El número de factura es obligatorio"))]
    pub numero_factura: String,
    /// Hora local del ticket, `AAAA-MM-DDTHH:MM:SS`
    #[validate(length(min = 1, message = "La fecha y hora son obligatorias"))]
    pub fecha_hora: String,
    pub total: f64,
    #[validate(length(max = 255))]
    pub tienda: Option<String>,
    #[validate(length(max = 255))]
    pub ubicacion: Option<String>,
    pub metodo_pago: Option<String>,
    #[validate(length(max = 100))]
    pub numero_operacion: Option<String>,
    #[validate(length(
        min = 1,
        max = 300,
        message = "El ticket debe tener entre 1 y 300 productos"
    ))]
    pub productos: Vec<TicketProduct>,
    #[serde(default)]
    pub iva_desglose: Vec<IvaBreakdown>,
}
