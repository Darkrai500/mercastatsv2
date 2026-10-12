//! Cliente mínimo de la API de Gmail (solo lectura) y extracción de PDFs de un mensaje.

use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use thiserror::Error;

const GMAIL_BASE_URL: &str = "https://gmail.googleapis.com";

/// Páginas de 500 ids como máximo al listar (tope de seguridad: 20.000 mensajes).
const MAX_LIST_PAGES: usize = 40;
/// Profundidad máxima del árbol MIME que se recorre (un correo hostil no puede desbordar la pila).
const MAX_MIME_DEPTH: usize = 12;
/// PDFs por mensaje que se consideran.
const MAX_PDFS_PER_MESSAGE: usize = 5;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GmailError {
    /// 401: el access token no vale
    #[error("Gmail rechazó las credenciales")]
    Unauthorized,
    /// 403: API de Gmail no habilitada en el proyecto, o ámbito insuficiente
    #[error("Gmail denegó el acceso")]
    Forbidden,
    #[error("mensaje no encontrado")]
    NotFound,
    /// Red, 429 o 5xx: se reintenta en la siguiente sincronización
    #[error("Gmail no disponible")]
    Unavailable,
    #[error("respuesta de Gmail no válida")]
    BadResponse,
}

// ---------------------------------------------------------------------------
// Tipos de la API (solo los campos que se usan)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ListResponse {
    #[serde(default)]
    messages: Vec<MessageRef>,
    #[serde(rename = "nextPageToken")]
    next_page_token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct MessageRef {
    id: String,
}

#[derive(Debug, Deserialize)]
pub struct GmailMessage {
    pub payload: Option<MessagePart>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MessagePart {
    pub mime_type: String,
    pub filename: String,
    pub headers: Vec<Header>,
    pub body: Option<PartBody>,
    pub parts: Vec<MessagePart>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Header {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct PartBody {
    pub attachment_id: Option<String>,
    pub size: Option<u64>,
    /// Contenido en línea (base64url); solo viene en partes pequeñas
    pub data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AttachmentResponse {
    #[serde(default)]
    data: String,
}

// ---------------------------------------------------------------------------
// Extracción de PDFs (lógica pura, sin red)
// ---------------------------------------------------------------------------

#[derive(Debug, PartialEq, Eq)]
pub enum PdfSource {
    /// Hay que descargarlo con `attachments.get`
    Attachment(String),
    /// Ya venía dentro del propio mensaje
    Inline(Vec<u8>),
}

#[derive(Debug, PartialEq, Eq)]
pub struct PdfAttachment {
    pub filename: String,
    pub size: Option<u64>,
    pub source: PdfSource,
}

/// Recorre el árbol MIME y devuelve los adjuntos PDF.
pub fn extract_pdf_attachments(root: &MessagePart) -> Vec<PdfAttachment> {
    let mut found = Vec::new();
    collect_pdfs(root, 0, &mut found);
    found
}

fn collect_pdfs(part: &MessagePart, depth: usize, found: &mut Vec<PdfAttachment>) {
    if depth > MAX_MIME_DEPTH || found.len() >= MAX_PDFS_PER_MESSAGE {
        return;
    }

    if is_pdf_part(part) {
        if let Some(body) = &part.body {
            let source = match (&body.attachment_id, &body.data) {
                (Some(id), _) => Some(PdfSource::Attachment(id.clone())),
                (None, Some(data)) => decode_base64url(data).map(PdfSource::Inline),
                (None, None) => None,
            };
            if let Some(source) = source {
                let filename = if part.filename.trim().is_empty() {
                    "ticket.pdf".to_string()
                } else {
                    part.filename.clone()
                };
                found.push(PdfAttachment {
                    filename,
                    size: body.size,
                    source,
                });
            }
        }
    }

    for child in &part.parts {
        collect_pdfs(child, depth + 1, found);
    }
}

fn is_pdf_part(part: &MessagePart) -> bool {
    let has_pdf_name = part.filename.to_ascii_lowercase().ends_with(".pdf");
    let is_pdf_mime = part.mime_type.eq_ignore_ascii_case("application/pdf");
    // Un adjunto sin nombre pero con MIME de PDF también cuenta; un nombre .pdf con MIME
    // genérico (application/octet-stream) también.
    has_pdf_name || (is_pdf_mime && part.parts.is_empty())
}

/// Dirección real del remitente (`Nombre <dir@dominio>` -> `dir@dominio`), en minúsculas.
///
/// La búsqueda `from:` de Gmail también casa con el nombre visible, que cualquiera puede poner
/// a su gusto; por eso se vuelve a comprobar la dirección de verdad antes de importar nada.
pub fn sender_address(root: &MessagePart) -> Option<String> {
    let from = root
        .headers
        .iter()
        .find(|h| h.name.eq_ignore_ascii_case("from"))?;
    let value = from.value.trim();
    let address = match (value.rfind('<'), value.rfind('>')) {
        (Some(start), Some(end)) if start < end => &value[start + 1..end],
        _ => value,
    };
    Some(address.trim().to_ascii_lowercase())
}

pub fn decode_base64url(data: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(data.trim().trim_end_matches('='))
        .ok()
}

// ---------------------------------------------------------------------------
// Cliente HTTP
// ---------------------------------------------------------------------------

pub struct GmailClient {
    http: Client,
    base_url: String,
}

impl GmailClient {
    pub fn new() -> Result<Self, reqwest::Error> {
        let http = Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            http,
            base_url: GMAIL_BASE_URL.to_string(),
        })
    }

    /// Solo para tests con una API de Gmail simulada.
    #[cfg(test)]
    pub fn with_base_url(mut self, base: &str) -> Self {
        self.base_url = base.trim_end_matches('/').to_string();
        self
    }

    /// Ids de los mensajes que cumplen la búsqueda, del más reciente al más antiguo.
    pub async fn list_message_ids(
        &self,
        access_token: &str,
        query: &str,
    ) -> Result<Vec<String>, GmailError> {
        let url = format!("{}/gmail/v1/users/me/messages", self.base_url);
        let mut ids = Vec::new();
        let mut page_token: Option<String> = None;

        for _ in 0..MAX_LIST_PAGES {
            let mut request = self
                .http
                .get(&url)
                .bearer_auth(access_token)
                .query(&[("q", query), ("maxResults", "500")]);
            if let Some(token) = &page_token {
                request = request.query(&[("pageToken", token.as_str())]);
            }

            let page: ListResponse = self
                .send(request)
                .await?
                .json()
                .await
                .map_err(|_| GmailError::BadResponse)?;
            ids.extend(page.messages.into_iter().map(|m| m.id));

            match page.next_page_token {
                Some(token) if !token.is_empty() => page_token = Some(token),
                _ => break,
            }
        }
        Ok(ids)
    }

    pub async fn get_message(
        &self,
        access_token: &str,
        message_id: &str,
    ) -> Result<GmailMessage, GmailError> {
        let url = format!(
            "{}/gmail/v1/users/me/messages/{}",
            self.base_url, message_id
        );
        let request = self
            .http
            .get(url)
            .bearer_auth(access_token)
            .query(&[("format", "full")]);
        self.send(request)
            .await?
            .json()
            .await
            .map_err(|_| GmailError::BadResponse)
    }

    pub async fn get_attachment(
        &self,
        access_token: &str,
        message_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<u8>, GmailError> {
        let url = format!(
            "{}/gmail/v1/users/me/messages/{}/attachments/{}",
            self.base_url, message_id, attachment_id
        );
        let request = self.http.get(url).bearer_auth(access_token);
        let body: AttachmentResponse = self
            .send(request)
            .await?
            .json()
            .await
            .map_err(|_| GmailError::BadResponse)?;
        decode_base64url(&body.data).ok_or(GmailError::BadResponse)
    }

    async fn send(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<reqwest::Response, GmailError> {
        let response = request.send().await.map_err(|_| GmailError::Unavailable)?;
        match response.status() {
            s if s.is_success() => Ok(response),
            StatusCode::UNAUTHORIZED => Err(GmailError::Unauthorized),
            StatusCode::FORBIDDEN => Err(GmailError::Forbidden),
            StatusCode::NOT_FOUND => Err(GmailError::NotFound),
            StatusCode::TOO_MANY_REQUESTS => Err(GmailError::Unavailable),
            s if s.is_server_error() => Err(GmailError::Unavailable),
            _ => Err(GmailError::BadResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse(value: serde_json::Value) -> MessagePart {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn encuentra_un_pdf_adjunto_en_un_multipart() {
        let mensaje = parse(json!({
            "mimeType": "multipart/mixed",
            "headers": [{"name": "From", "value": "Mercadona <ticket_digital@mail.mercadona.com>"}],
            "parts": [
                {"mimeType": "text/html", "filename": "", "body": {"size": 10, "data": "PGI-aG9sYTwvYj4"}},
                {"mimeType": "application/pdf", "filename": "ticket.pdf",
                 "body": {"attachmentId": "ATT1", "size": 4321}}
            ]
        }));
        assert_eq!(
            extract_pdf_attachments(&mensaje),
            vec![PdfAttachment {
                filename: "ticket.pdf".into(),
                size: Some(4321),
                source: PdfSource::Attachment("ATT1".into()),
            }]
        );
    }

    #[test]
    fn recorre_multiparts_anidados() {
        let mensaje = parse(json!({
            "mimeType": "multipart/mixed",
            "parts": [{
                "mimeType": "multipart/related",
                "parts": [{
                    "mimeType": "multipart/alternative",
                    "parts": [{"mimeType": "application/pdf", "filename": "Profundo.PDF",
                               "body": {"attachmentId": "DEEP"}}]
                }]
            }]
        }));
        let pdfs = extract_pdf_attachments(&mensaje);
        assert_eq!(pdfs.len(), 1);
        assert_eq!(pdfs[0].source, PdfSource::Attachment("DEEP".into()));
    }

    #[test]
    fn acepta_pdf_con_mime_generico_si_el_nombre_acaba_en_pdf() {
        let mensaje = parse(json!({
            "parts": [{"mimeType": "application/octet-stream", "filename": "factura.pdf",
                       "body": {"attachmentId": "X"}}]
        }));
        assert_eq!(extract_pdf_attachments(&mensaje).len(), 1);
    }

    #[test]
    fn acepta_pdf_en_linea_y_lo_decodifica() {
        // "%PDF-1.4" en base64url
        let mensaje = parse(json!({
            "parts": [{"mimeType": "application/pdf", "filename": "x.pdf",
                       "body": {"data": "JVBERi0xLjQ"}}]
        }));
        let pdfs = extract_pdf_attachments(&mensaje);
        assert_eq!(pdfs[0].source, PdfSource::Inline(b"%PDF-1.4".to_vec()));
    }

    #[test]
    fn ignora_adjuntos_que_no_son_pdf() {
        let mensaje = parse(json!({
            "parts": [
                {"mimeType": "image/png", "filename": "logo.png", "body": {"attachmentId": "A"}},
                {"mimeType": "text/plain", "filename": "", "body": {"data": "aG9sYQ"}}
            ]
        }));
        assert!(extract_pdf_attachments(&mensaje).is_empty());
    }

    #[test]
    fn un_pdf_sin_nombre_recibe_uno_por_defecto() {
        let mensaje = parse(json!({
            "parts": [{"mimeType": "application/pdf", "filename": "", "body": {"attachmentId": "A"}}]
        }));
        assert_eq!(extract_pdf_attachments(&mensaje)[0].filename, "ticket.pdf");
    }

    #[test]
    fn limita_la_cantidad_de_pdfs_por_mensaje() {
        let partes: Vec<_> = (0..20)
            .map(|i| {
                json!({"mimeType": "application/pdf", "filename": format!("{i}.pdf"),
                            "body": {"attachmentId": format!("A{i}")}})
            })
            .collect();
        let mensaje = parse(json!({ "parts": partes }));
        assert_eq!(
            extract_pdf_attachments(&mensaje).len(),
            MAX_PDFS_PER_MESSAGE
        );
    }

    #[test]
    fn no_se_desborda_con_un_arbol_mime_hostil() {
        // 200 niveles de anidación
        let mut valor = json!({"mimeType": "application/pdf", "filename": "x.pdf",
                               "body": {"attachmentId": "A"}});
        for _ in 0..200 {
            valor = json!({"mimeType": "multipart/mixed", "parts": [valor]});
        }
        assert!(extract_pdf_attachments(&parse(valor)).is_empty());
    }

    #[test]
    fn extrae_la_direccion_real_del_remitente() {
        let con_nombre = parse(json!({"headers": [
            {"name": "from", "value": "\"Mercadona S.A.\" <Ticket_Digital@Mail.Mercadona.com>"}]}));
        assert_eq!(
            sender_address(&con_nombre).as_deref(),
            Some("ticket_digital@mail.mercadona.com")
        );

        let solo_direccion = parse(json!({"headers": [
            {"name": "From", "value": "ticket_digital@mail.mercadona.com"}]}));
        assert_eq!(
            sender_address(&solo_direccion).as_deref(),
            Some("ticket_digital@mail.mercadona.com")
        );

        assert_eq!(sender_address(&parse(json!({"headers": []}))), None);
    }

    #[test]
    fn un_nombre_visible_falso_no_cuenta_como_remitente() {
        // El nombre visible imita al remitente legítimo, pero la dirección real es otra
        let falso = parse(json!({"headers": [
            {"name": "From", "value": "ticket_digital@mail.mercadona.com <atacante@evil.example>"}]}));
        assert_eq!(
            sender_address(&falso).as_deref(),
            Some("atacante@evil.example")
        );
    }

    #[test]
    fn decodifica_base64url_con_y_sin_relleno() {
        assert_eq!(decode_base64url("aG9sYQ").unwrap(), b"hola");
        assert_eq!(decode_base64url("aG9sYQ==").unwrap(), b"hola");
        assert_eq!(decode_base64url("_-8").unwrap(), vec![0xff, 0xef]);
        assert!(decode_base64url("***").is_none());
    }
}
