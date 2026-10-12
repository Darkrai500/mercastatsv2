use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Tamaño máximo del archivo de un ticket (PDF o foto)
pub const MAX_TICKET_FILE_BYTES: usize = 10_485_760; // 10 MB

/// Extensiones de foto que admite la tabla `tickets_pdf` (migración 0004)
pub const PHOTO_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "heic", "heif"];

/// Extensión con la que se guarda una foto de cada tipo si su nombre no trae una válida
pub fn photo_extension(mime: &str) -> &'static str {
    match mime {
        "image/png" => "png",
        "image/webp" => "webp",
        "image/heic" => "heic",
        _ => "jpg",
    }
}

/// Marcas `ftyp` de los contenedores HEIF/HEIC (fotos de iPhone)
const HEIF_BRANDS: [&[u8; 4]; 8] = [
    b"heic", b"heix", b"hevc", b"hevx", b"heim", b"heis", b"mif1", b"msf1",
];

/// Modelo de dominio para un PDF de ticket almacenado
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketPdf {
    pub numero_factura: String,
    pub ticket_pdf: Vec<u8>,
    pub ticket_nombre_archivo: String,
    pub ticket_tamano_bytes: i32,
    pub created_at: NaiveDateTime,
}

/// DTO para insertar el archivo de un ticket: el PDF digital o la foto del ticket en papel
/// (la tabla conserva el nombre `tickets_pdf` de cuando solo había PDF).
#[derive(Debug, Clone)]
pub struct TicketPdfInsert {
    pub numero_factura: String,
    pub ticket_pdf: Vec<u8>,
    pub ticket_nombre_archivo: String,
}

/// Tipo MIME de un archivo de ticket según su contenido (bytes mágicos), no según la
/// extensión o el MIME que declara el cliente. `None` si no es un PDF ni una foto soportada.
pub fn detect_ticket_mime(bytes: &[u8]) -> Option<&'static str> {
    let head = &bytes[..bytes.len().min(1024)];
    if head.windows(5).any(|w| w == b"%PDF-") {
        Some("application/pdf")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.len() >= 12 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.len() >= 12
        && &bytes[4..8] == b"ftyp"
        && HEIF_BRANDS.iter().any(|brand| &bytes[8..12] == *brand)
    {
        Some("image/heic")
    } else {
        None
    }
}

impl TicketPdfInsert {
    /// Valida que el archivo tenga extensión .pdf
    pub fn validate_pdf_extension(filename: &str) -> bool {
        filename.to_lowercase().ends_with(".pdf")
    }

    /// Valida que una foto tenga extensión de imagen (lo exige la tabla, migración 0004)
    pub fn validate_photo_extension(filename: &str) -> bool {
        let lower = filename.to_lowercase();
        PHOTO_EXTENSIONS
            .iter()
            .any(|ext| lower.ends_with(&format!(".{ext}")))
    }

    /// Valida que el tamaño sea menor o igual a 10MB
    pub fn validate_size(size: usize) -> bool {
        size <= MAX_TICKET_FILE_BYTES
    }

    /// Calcula el tamaño en bytes
    pub fn calculate_size(&self) -> i32 {
        self.ticket_pdf.len() as i32
    }

    /// Valida el archivo completo: un PDF (como siempre, por su extensión) o una foto
    /// reconocida por su contenido, de 10 MB como máximo
    pub fn validate(&self) -> Result<(), String> {
        let is_photo =
            detect_ticket_mime(&self.ticket_pdf).is_some_and(|mime| mime.starts_with("image/"));
        let valid_name = if is_photo {
            Self::validate_photo_extension(&self.ticket_nombre_archivo)
        } else {
            Self::validate_pdf_extension(&self.ticket_nombre_archivo)
        };
        if !valid_name {
            return Err(
                "El archivo debe ser un PDF o una foto (JPG, PNG, WEBP o HEIC)".to_string(),
            );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn insert(bytes: &[u8], nombre: &str) -> TicketPdfInsert {
        TicketPdfInsert {
            numero_factura: "0000-000-000001".into(),
            ticket_pdf: bytes.to_vec(),
            ticket_nombre_archivo: nombre.into(),
        }
    }

    #[test]
    fn detecta_pdf_y_fotos_por_su_contenido() {
        assert_eq!(
            detect_ticket_mime(b"%PDF-1.7\n..."),
            Some("application/pdf")
        );
        assert_eq!(
            detect_ticket_mime(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]),
            Some("image/jpeg")
        );
        assert_eq!(
            detect_ticket_mime(b"\x89PNG\r\n\x1a\n\0\0"),
            Some("image/png")
        );
        assert_eq!(
            detect_ticket_mime(b"RIFF\0\0\0\0WEBPVP8 "),
            Some("image/webp")
        );
        assert_eq!(
            detect_ticket_mime(b"\0\0\0\x18ftypheic\0\0\0\0"),
            Some("image/heic")
        );
        assert_eq!(detect_ticket_mime(b"PK\x03\x04zip"), None);
        assert_eq!(detect_ticket_mime(b"<svg></svg>"), None);
        assert_eq!(detect_ticket_mime(b""), None);
    }

    #[test]
    fn acepta_fotos_por_su_contenido_con_extension_de_imagen() {
        assert!(insert(&[0xFF, 0xD8, 0xFF, 0xE1], "IMG_0001.jpg")
            .validate()
            .is_ok());
        assert!(insert(b"\0\0\0\x18ftypheic", "IMG_0002.HEIC")
            .validate()
            .is_ok());
        // La tabla exige una extensión válida: sin ella fallaría al insertar
        assert!(insert(&[0xFF, 0xD8, 0xFF, 0xE1], "foto")
            .validate()
            .is_err());
    }

    #[test]
    fn los_pdf_se_validan_como_siempre_por_su_extension() {
        assert!(insert(b"%PDF-1.4\n", "ticket.pdf").validate().is_ok());
        assert!(insert(b"cualquier cosa", "ticket.PDF").validate().is_ok());
        assert!(insert(b"%PDF-1.4\n", "ticket").validate().is_err());
    }

    #[test]
    fn rechaza_otros_formatos_y_archivos_demasiado_grandes() {
        assert!(insert(b"MZ\x90\x00", "programa.exe").validate().is_err());

        let mut grande = vec![0xFF, 0xD8, 0xFF];
        grande.resize(MAX_TICKET_FILE_BYTES + 1, 0);
        assert!(insert(&grande, "foto.jpg")
            .validate()
            .unwrap_err()
            .contains("10MB"));
    }
}
