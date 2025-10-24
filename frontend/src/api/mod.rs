pub mod auth;
pub mod tickets;

use serde::{Deserialize, Serialize};

/// URL base del backend
pub const API_BASE_URL: &str = "http://localhost:8000/api";

/// Estructura de error estándar de la API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}

/// Obtener el token de autenticación del localStorage
pub fn get_auth_token() -> Option<String> {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            if let Ok(Some(token)) = storage.get_item("auth_token") {
                return Some(token);
            }
        }
    }
    None
}
