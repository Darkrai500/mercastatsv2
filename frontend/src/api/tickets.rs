use serde::{Deserialize, Serialize};
use gloo_net::http::Request;
use web_sys::File;
use wasm_bindgen::JsValue;
use super::{API_BASE_URL, ApiError, get_auth_token};

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

/// Subir un ticket (PDF o imagen)
pub async fn upload_ticket(file: File) -> Result<UploadResponse, String> {
    let url = format!("{}/tickets/upload", API_BASE_URL);

    // Obtener token de autenticación
    let token = get_auth_token()
        .ok_or_else(|| "No hay sesión activa".to_string())?;

    // Crear FormData
    let form_data = web_sys::FormData::new()
        .map_err(|_| "Error al crear FormData".to_string())?;

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

/// Obtener lista de tickets del usuario
pub async fn get_user_tickets() -> Result<Vec<Ticket>, String> {
    let url = format!("{}/tickets", API_BASE_URL);

    let token = get_auth_token()
        .ok_or_else(|| "No hay sesión activa".to_string())?;

    let response = Request::get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Error de conexión: {}", e))?;

    if response.ok() {
        response
            .json::<Vec<Ticket>>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();

        if status == 401 {
            return Err("Sesión expirada. Por favor, inicia sesión nuevamente.".to_string());
        }

        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo obtener tickets", status));
        Err(error)
    }
}
