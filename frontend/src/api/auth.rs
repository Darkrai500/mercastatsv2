use super::{ApiError, API_BASE_URL};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoginResponse {
    pub token: String,
    pub user: UserInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserInfo {
    pub email: String,
    pub nombre: Option<String>,
    #[serde(default)]
    pub is_demo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub email: String,
    pub password: String,
    pub nombre: Option<String>,
}

/// Iniciar sesión
pub async fn login_user(credentials: LoginRequest) -> Result<LoginResponse, String> {
    let url = format!("{}/auth/login", API_BASE_URL);

    let response = Request::post(&url)
        .json(&credentials)
        .map_err(|e| format!("Error al preparar petición: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Error de conexión: {}", e))?;

    if response.ok() {
        response
            .json::<LoginResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo iniciar sesión", status));
        Err(error)
    }
}

/// Registrar nuevo usuario
pub async fn register_user(data: RegisterRequest) -> Result<LoginResponse, String> {
    let url = format!("{}/auth/register", API_BASE_URL);

    let response = Request::post(&url)
        .json(&data)
        .map_err(|e| format!("Error al preparar petición: {}", e))?
        .send()
        .await
        .map_err(|e| format!("Error de conexión: {}", e))?;

    if response.ok() {
        response
            .json::<LoginResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo registrar", status));
        Err(error)
    }
}

/// Cerrar sesión (del lado del cliente)
pub fn logout() {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.local_storage() {
            let _ = storage.remove_item("auth_token");
            let _ = storage.remove_item("user_email");
        }
    }
}
