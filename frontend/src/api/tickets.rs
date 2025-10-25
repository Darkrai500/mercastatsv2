use super::{get_auth_token, ApiError, API_BASE_URL};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use web_sys::File;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResponse {
    pub ticket_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ticket {
    pub numero_factura: String,
    pub fecha: String,
    pub total: f64,
    pub numero_productos: i32,
}

// ===== Estructuras para OCR =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTicketRequest {
    pub ticket_id: String,
    pub file_name: String,
    pub pdf_b64: String,
    pub usuario_email: Option<String>,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IvaBreakdown {
    pub porcentaje: f64,
    pub base_imponible: f64,
    pub cuota: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrResponseSummary {
    pub ticket_id: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub total: Option<f64>,
    pub productos_detectados: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketIngestionResponse {
    pub ingested: bool,
    pub numero_factura: String,
    pub total: String,
    pub productos_insertados: usize,
    pub fecha_hora: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessTicketResponse {
    pub ocr: OcrResponseSummary,
    pub ingestion: Option<TicketIngestionResponse>,
}

// Legacy response para compatibilidad con código existente
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyProcessTicketResponse {
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
}

/// Subir un ticket (PDF o imagen)
pub async fn upload_ticket(file: File) -> Result<UploadResponse, String> {
    let url = format!("{}/tickets/upload", API_BASE_URL);

    // Obtener token de autenticación
    let token = get_auth_token().ok_or_else(|| "No hay sesión activa".to_string())?;

    // Crear FormData
    let form_data = web_sys::FormData::new().map_err(|_| "Error al crear FormData".to_string())?;

    form_data
        .append_with_blob("file", &file)
        .map_err(|_| "Error al agregar archivo".to_string())?;

    // Convertir FormData a JsValue para gloo-net
    let form_data_value: JsValue = form_data.into();

    let response = Request::post(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .body(form_data_value)
        .map_err(|e| format!("Error al preparar petición: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Error de conexión: {}", e))?;

    if response.ok() {
        response
            .json::<UploadResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();

        // Si es 401, sesión expirada
        if status == 401 {
            return Err("Sesión expirada. Por favor, inicia sesión nuevamente.".to_string());
        }

        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo subir el ticket", status));
        Err(error)
    }
}

// ===== Estructuras para histórico de tickets =====

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketHistoryItem {
    pub numero_factura: String,
    pub fecha_hora: String,
    pub total: String,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub num_productos: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub total_tickets: Option<i64>,
    pub total_gastado: Option<String>,
    pub productos_unicos: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketHistoryResponse {
    pub tickets: Vec<TicketHistoryItem>,
    pub stats: UserStats,
}

/// Obtener histórico de tickets del usuario
pub async fn get_user_ticket_history(usuario_email: &str) -> Result<TicketHistoryResponse, String> {
    let url = format!(
        "{}/tickets/history?usuario_email={}",
        API_BASE_URL, usuario_email
    );

    let response = Request::get(&url)
        .send()
        .await
        .map_err(|e| format!("Error de conexión: {}", e))?;

    if response.ok() {
        response
            .json::<TicketHistoryResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo obtener el histórico", status));
        Err(error)
    }
}

/// Procesar ticket con OCR (PDF o imagen) e ingestarlo en la base de datos
pub async fn process_ticket_ocr(file: File) -> Result<ProcessTicketResponse, String> {
    let url = format!("{}/ocr/process", API_BASE_URL);

    // Generar ID único para el ticket
    let ticket_id = format!("ticket_{}", js_sys::Date::now() as u64);
    let file_name = file.name();

    // Convertir archivo a base64
    let pdf_b64 = file_to_base64(&file).await?;

    // Obtener email del usuario de localStorage
    let usuario_email = if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            storage.get_item("user_email").ok().flatten()
        } else {
            None
        }
    } else {
        None
    };

    let request_body = ProcessTicketRequest {
        ticket_id,
        file_name,
        pdf_b64,
        usuario_email,
    };

    let response = Request::post(&url)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .map_err(|e| format!("Error al preparar petición: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Error de conexión: {}", e))?;

    if response.ok() {
        response
            .json::<ProcessTicketResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();

        // Capturar errores específicos (como duplicados)
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo procesar el ticket", status));
        Err(error)
    }
}

/// Convierte un archivo a base64
async fn file_to_base64(file: &File) -> Result<String, String> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;
    use std::rc::Rc;
    use std::cell::RefCell;

    let file_reader = web_sys::FileReader::new().map_err(|_| "Error al crear FileReader")?;
    let file_reader_rc = Rc::new(file_reader);

    let result_holder: Rc<RefCell<Option<String>>> = Rc::new(RefCell::new(None));
    let result_holder_clone = result_holder.clone();

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let file_reader_clone = file_reader_rc.clone();
        let onload = wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Ok(result) = file_reader_clone.result() {
                resolve.call1(&JsValue::NULL, &result).unwrap();
            }
        }) as Box<dyn FnMut(_)>);

        let onerror = wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::Event| {
            reject.call1(&JsValue::NULL, &JsValue::from_str("Error al leer archivo")).unwrap();
        }) as Box<dyn FnMut(_)>);

        file_reader_rc.set_onload(Some(onload.as_ref().unchecked_ref()));
        file_reader_rc.set_onerror(Some(onerror.as_ref().unchecked_ref()));

        onload.forget();
        onerror.forget();
    });

    file_reader_rc
        .read_as_data_url(file)
        .map_err(|_| "Error al leer archivo")?;

    let result = JsFuture::from(promise)
        .await
        .map_err(|_| "Error al esperar lectura de archivo")?;

    let data_url = result
        .as_string()
        .ok_or_else(|| "Error al convertir resultado a string")?;

    // Extraer solo la parte base64 (después de "data:...;base64,")
    let base64 = data_url
        .split(',')
        .nth(1)
        .ok_or_else(|| "Error al extraer base64")?
        .to_string();

    Ok(base64)
}
