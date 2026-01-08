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
    pub mime_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IvaBreakdown {
    pub porcentaje: f64,
    pub base_imponible: f64,
    pub cuota: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OcrResponseSummary {
    pub ticket_id: String,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    #[serde(deserialize_with = "option_f64_from_number_or_string")]
    pub total: Option<f64>,
    pub productos_detectados: usize,
    #[serde(default)]
    pub productos: Vec<TicketProduct>,
    #[serde(default)]
    pub tienda: Option<String>,
    #[serde(default)]
    pub processing_profile: Option<String>,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(default)]
    pub raw_text_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TicketIngestionResponse {
    #[serde(default)]
    pub ingested: bool,
    #[serde(default)]
    pub ticket_id: Option<String>,
    #[serde(default)]
    pub raw_text: Option<String>,
    pub numero_factura: Option<String>,
    pub fecha: Option<String>,
    pub fecha_hora: Option<String>,
    #[serde(deserialize_with = "option_f64_from_number_or_string")]
    pub total: Option<f64>,
    #[serde(default)]
    pub tienda: Option<String>,
    #[serde(default)]
    pub ubicacion: Option<String>,
    #[serde(default)]
    pub metodo_pago: Option<String>,
    #[serde(default)]
    pub numero_operacion: Option<String>,
    #[serde(default)]
    pub productos: Vec<TicketProduct>,
    #[serde(default)]
    pub iva_desglose: Vec<IvaBreakdown>,
    #[serde(default)]
    pub productos_insertados: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessTicketResponse {
    pub ocr: OcrResponseSummary,
    pub ingestion: Option<TicketIngestionResponse>,
}

// Legacy response para compatibilidad con codigo existente
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

    // Obtener token de autenticacion
    let token = get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())?;

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
        .map_err(|e| format!("Error al preparar peticion: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;

    if response.ok() {
        response
            .json::<UploadResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo subir el ticket", status));
        Err(error)
    }
}

/// Obtener el historico de tickets y estadisticas del usuario autenticado
pub async fn get_user_ticket_history() -> Result<TicketHistoryResponse, String> {
    let token = get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())?;
    let url = format!("{}/tickets/history", API_BASE_URL);

    let response = Request::get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;

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
            .unwrap_or_else(|_| format!("Error {}: No se pudo obtener el historico", status));
        Err(error)
    }
}

/// Procesar ticket con OCR (PDF o imagen) e ingestarlo en la base de datos
pub async fn process_ticket_ocr(file: File, ingest: bool) -> Result<ProcessTicketResponse, String> {
    let url = format!("{}/ocr/process", API_BASE_URL);
    let token = get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())?;

    // Generar ID unico para el ticket
    let ticket_id = format!("ticket_{}", js_sys::Date::now() as u64);
    let file_name = file.name();
    let mime_type = {
        let mt = file.type_();
        if mt.is_empty() { None } else { Some(mt) }
    };

    // Convertir archivo a base64
    let pdf_b64 = file_to_base64(&file).await?;

    // Obtener email del usuario de localStorage solo si vamos a ingerir
    let usuario_email = if ingest {
        if let Some(window) = web_sys::window() {
            if let Ok(Some(storage)) = window.local_storage() {
                storage.get_item("user_email").ok().flatten()
            } else {
                None
            }
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
        mime_type,
    };

    let response = Request::post(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .json(&request_body)
        .map_err(|e| format!("Error al preparar peticion: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;

    if response.ok() {
        response
            .json::<ProcessTicketResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();

        // Capturar errores especificos (como duplicados)
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo procesar el ticket", status));
        Err(error)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketHistoryItem {
    pub numero_factura: String,
    pub fecha_hora: String,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub total: String,
    pub num_productos: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStats {
    pub total_tickets: Option<i64>,
    pub total_gastado: Option<String>,
    pub gasto_medio: Option<String>,
    pub productos_unicos: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TicketHistoryResponse {
    pub tickets: Vec<TicketHistoryItem>,
    pub stats: UserStats,
}

fn option_f64_from_number_or_string<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{Error, Unexpected};

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum NumOrString {
        Num(f64),
        Str(String),
        Null,
    }

    match NumOrString::deserialize(deserializer)? {
        NumOrString::Num(n) => Ok(Some(n)),
        NumOrString::Str(s) => s
            .replace(',', ".")
            .parse::<f64>()
            .map(Some)
            .map_err(|_| Error::invalid_value(Unexpected::Str(&s), &"un numero valido")),
        NumOrString::Null => Ok(None),
    }
}

/// Convierte un archivo a base64
async fn file_to_base64(file: &File) -> Result<String, String> {
    use std::rc::Rc;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let file_reader = web_sys::FileReader::new().map_err(|_| "Error al crear FileReader")?;
    let file_reader_rc = Rc::new(file_reader);

    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let file_reader_clone = file_reader_rc.clone();
        let onload = wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::Event| {
            if let Ok(result) = file_reader_clone.result() {
                resolve.call1(&JsValue::NULL, &result).unwrap();
            }
        }) as Box<dyn FnMut(_)>);

        let onerror =
            wasm_bindgen::closure::Closure::wrap(Box::new(move |_event: web_sys::Event| {
                reject
                    .call1(&JsValue::NULL, &JsValue::from_str("Error al leer archivo"))
                    .unwrap();
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

    // Extraer solo la parte base64 (despues de \"data:...;base64,\")
    let base64 = data_url
        .split(',')
        .nth(1)
        .ok_or_else(|| "Error al extraer base64")?
        .to_string();

    Ok(base64)
}
