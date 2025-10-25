use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Modelo de dominio para un PDF de ticket almacenado
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketPdf {
    pub numero_factura: String,
    pub ticket_pdf: Vec<u8>,
    pub ticket_nombre_archivo: String,
    pub ticket_tamano_bytes: i32,
    pub created_at: NaiveDateTime,
}

/// DTO para insertar un PDF de ticket
#[derive(Debug, Clone)]
pub struct TicketPdfInsert {
    pub numero_factura: String,
    pub ticket_pdf: Vec<u8>,
    pub ticket_nombre_archivo: String,
}

impl TicketPdfInsert {
    /// Valida que el archivo tenga extensión .pdf
    pub fn validate_pdf_extension(filename: &str) -> bool {
        filename.to_lowercase().ends_with(".pdf")
    }

    /// Valida que el tamaño sea menor o igual a 10MB
    pub fn validate_size(size: usize) -> bool {
        size <= 10_485_760 // 10 MB
    }

    /// Calcula el tamaño en bytes
    pub fn calculate_size(&self) -> i32 {
        self.ticket_pdf.len() as i32
    }

    /// Valida el PDF completo
    pub fn validate(&self) -> Result<(), String> {
        if !Self::validate_pdf_extension(&self.ticket_nombre_archivo) {
            return Err("El archivo debe tener extensión .pdf".to_string());
        }

        let size = self.ticket_pdf.len();
        if !Self::validate_size(size) {
            return Err(format!(
                "El archivo excede el tamaño máximo de 10MB (tamaño actual: {} bytes)",
                size
            ));
        }

        Ok(())
    }
}
