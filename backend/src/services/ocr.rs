use serde::{Deserialize, Serialize};

/// Request para procesamiento OCR a traves del servicio externo de inteligencia.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrProcessTicketRequest {
    pub ticket_id: String,
    pub file_name: String,
    #[serde(rename = "file_content_b64")]
    pub file_content_b64: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Producto detectado en el ticket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketProduct {
    pub nombre: String,
    pub cantidad: f64,
    pub unidad: String,
    pub precio_unitario: f64,
    pub precio_total: f64,
    #[serde(default)]
    pub descuento: f64,
    #[serde(default)]
    pub iva_porcentaje: f64,
    #[serde(default)]
    pub iva_importe: f64,
}

/// Desglose de IVA reportado en el ticket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IvaBreakdown {
    pub porcentaje: f64,
    pub base_imponible: f64,
    pub cuota: f64,
}

/// Respuesta completa del procesamiento OCR.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrProcessTicketResponse {
    pub ticket_id: String,
    pub raw_text: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub fecha_hora: Option<String>,
    pub total: Option<f64>,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub metodo_pago: Option<String>,
    pub numero_operacion: Option<String>,
    #[serde(default)]
    pub productos: Vec<TicketProduct>,
    #[serde(default)]
    pub iva_desglose: Vec<IvaBreakdown>,
    #[serde(default)]
    pub processing_profile: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
}
