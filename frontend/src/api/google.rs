//! Inicio de sesión con Google y sincronización de tickets desde Gmail.

use super::{get_auth_token, ApiError, API_BASE_URL};
use gloo_net::http::Request;
use serde::Deserialize;

/// Dónde se guarda un aviso para mostrarlo tras volver de Google (la navegación recarga la app).
pub const FLASH_KEY: &str = "google_flash";

#[derive(Debug, Clone, Deserialize)]
pub struct Providers {
    pub google: bool,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct GmailStatus {
    pub disponible: bool,
    pub conectado: bool,
    #[serde(default)]
    pub remitente: Option<String>,
    #[serde(default)]
    pub google_email: Option<String>,
    /// "activa" o "requiere_reconexion"
    #[serde(default)]
    pub estado: Option<String>,
    #[serde(default)]
    pub sincronizando: bool,
    #[serde(default)]
    pub ultima_sync_fin: Option<String>,
    #[serde(default)]
    pub ultima_sync_ok: Option<bool>,
    #[serde(default)]
    pub ultima_sync_error: Option<String>,
    #[serde(default)]
    pub ultima_sync_importados: i32,
    #[serde(default)]
    pub ultima_sync_duplicados: i32,
    #[serde(default)]
    pub ultima_sync_fallidos: i32,
}

#[derive(Debug, Clone, Deserialize)]
struct ConnectResponse {
    url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncStarted {
    pub iniciada: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Disconnected {
    pub revocado_en_google: bool,
}

/// Traduce un código de error del backend (fragmento `#error=…` o `ultima_sync_error`).
pub fn error_message(code: &str) -> String {
    match code {
        "acceso_denegado" => "Has cancelado el acceso con Google.",
        "permiso_gmail_denegado" => {
            "Para importar tus tickets necesitamos permiso de lectura de Gmail. Vuelve a intentarlo y deja marcada esa casilla."
        }
        "cuenta_existente" => {
            "Ya existe una cuenta con ese email. Inicia sesión con tu contraseña y conecta Google desde «Subir tickets»."
        }
        "google_en_uso" => "Esa cuenta de Google ya está conectada a otro usuario.",
        "estado_invalido" => "La sesión de inicio con Google ha caducado. Inténtalo de nuevo.",
        "codigo_invalido" => "Google rechazó el código de acceso. Inténtalo de nuevo.",
        "email_no_verificado" => "Tu cuenta de Google no tiene el email verificado.",
        "sin_refresh_token" => {
            "Google no nos dio acceso permanente a tu Gmail. Revoca Mercastats en myaccount.google.com/permissions y vuelve a conectar."
        }
        "no_configurado" => "El inicio de sesión con Google no está configurado en este servidor.",
        "demo" => "La cuenta de demostración no puede conectar Gmail.",
        "google_no_disponible" => "Google no responde ahora mismo. Inténtalo en unos minutos.",
        // Errores de la última sincronización
        "token_revocado" | "clave_cifrado_invalida" => {
            "El acceso a tu Gmail ya no es válido. Vuelve a conectarlo."
        }
        "gmail_denegado" => {
            "Gmail denegó el acceso. Si eres el administrador, comprueba que la API de Gmail está habilitada en Google Cloud."
        }
        "ocr_no_disponible" => "El lector de tickets no está disponible. Se reintentará solo.",
        "google_rechazado" => "Google rechazó las credenciales de la aplicación. Revisa GOOGLE_CLIENT_ID/SECRET.",
        _ => "Ha ocurrido un error inesperado. Inténtalo de nuevo.",
    }
    .to_string()
}

fn session_token() -> Result<String, String> {
    get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())
}

async fn read_error(response: gloo_net::http::Response, fallback: &str) -> String {
    let status = response.status();
    response
        .json::<ApiError>()
        .await
        .map(|e| e.error)
        .unwrap_or_else(|_| format!("Error {}: {}", status, fallback))
}

/// Qué proveedores de inicio de sesión hay activos (no requiere sesión).
pub async fn get_providers() -> Result<Providers, String> {
    let response = Request::get(&format!("{}/auth/providers", API_BASE_URL))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;
    if response.ok() {
        response
            .json::<Providers>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        Err(read_error(response, "No se pudo consultar los proveedores").await)
    }
}

pub async fn gmail_status() -> Result<GmailStatus, String> {
    let response = Request::get(&format!("{}/gmail/status", API_BASE_URL))
        .header("Authorization", &format!("Bearer {}", session_token()?))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;
    if response.ok() {
        response
            .json::<GmailStatus>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        Err(read_error(response, "No se pudo consultar Gmail").await)
    }
}

/// URL de Google a la que hay que navegar para conectar la cuenta.
pub async fn gmail_connect_url() -> Result<String, String> {
    let response = Request::post(&format!("{}/gmail/connect", API_BASE_URL))
        .header("Authorization", &format!("Bearer {}", session_token()?))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;
    if response.ok() {
        response
            .json::<ConnectResponse>()
            .await
            .map(|r| r.url)
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        Err(read_error(response, "No se pudo iniciar la conexion").await)
    }
}

pub async fn gmail_sync_now() -> Result<SyncStarted, String> {
    let response = Request::post(&format!("{}/gmail/sync", API_BASE_URL))
        .header("Authorization", &format!("Bearer {}", session_token()?))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;
    if response.ok() {
        response
            .json::<SyncStarted>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        Err(read_error(response, "No se pudo iniciar la sincronizacion").await)
    }
}

pub async fn gmail_disconnect() -> Result<Disconnected, String> {
    let response = Request::delete(&format!("{}/gmail/connection", API_BASE_URL))
        .header("Authorization", &format!("Bearer {}", session_token()?))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;
    if response.ok() {
        response
            .json::<Disconnected>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        Err(read_error(response, "No se pudo desconectar").await)
    }
}

/// Aviso pendiente tras volver de Google.
#[derive(Debug, Clone, PartialEq)]
pub struct Flash {
    pub is_error: bool,
    pub message: String,
}

/// Guarda un aviso para mostrarlo en la página a la que se navega a continuación.
pub fn set_flash(message: &str, is_error: bool) {
    if let Some(window) = web_sys::window() {
        if let Ok(Some(storage)) = window.session_storage() {
            let kind = if is_error { "err" } else { "ok" };
            let _ = storage.set_item(FLASH_KEY, &format!("{kind}:{message}"));
        }
    }
}

/// Recupera y borra el aviso pendiente, si lo hay.
pub fn take_flash() -> Option<Flash> {
    let storage = web_sys::window()?.session_storage().ok()??;
    let raw = storage.get_item(FLASH_KEY).ok()??;
    let _ = storage.remove_item(FLASH_KEY);
    let (kind, message) = raw.split_once(':')?;
    Some(Flash {
        is_error: kind == "err",
        message: message.to_string(),
    })
}
