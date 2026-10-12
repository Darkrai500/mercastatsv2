//! Gestión de la conexión con Gmail del usuario autenticado.
//!
//! - `POST   /api/gmail/connect`     inicia la vinculación (devuelve la URL de Google)
//! - `GET    /api/gmail/status`      estado de la conexión y de la última sincronización
//! - `POST   /api/gmail/sync`        sincroniza ahora (en segundo plano)
//! - `DELETE /api/gmail/connection`  revoca el acceso en Google y borra el token

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;

use super::{auth::AppState, google_auth::begin_authorization};
use crate::{
    db,
    error::{AppError, AppResult},
    middleware::AuthenticatedUser,
    models::google_account::ESTADO_ACTIVA,
    services::gmail_sync::{spawn_sync, SyncError},
};

fn not_configured() -> AppError {
    AppError::BadRequest("La integración con Google no está configurada".to_string())
}

#[derive(Serialize)]
pub struct ConnectResponse {
    pub url: String,
}

/// Devuelve la URL de Google a la que el frontend debe navegar. Es un POST autenticado (y no
/// un enlace directo) porque el navegador no puede mandar la cabecera `Authorization` en una
/// navegación; así la vinculación queda atada al usuario con sesión iniciada.
pub async fn connect(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Response> {
    if auth_user.is_demo {
        return Err(AppError::DemoUserRestriction);
    }
    let google = state.google.as_ref().ok_or_else(not_configured)?;
    let (url, cookie) = begin_authorization(&state, google, Some(auth_user.email), false, false)?;
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(ConnectResponse { url }),
    )
        .into_response())
}

#[derive(Serialize)]
pub struct GmailStatus {
    /// La integración está configurada en el servidor
    pub disponible: bool,
    pub conectado: bool,
    /// Remitente cuyos correos se importan
    pub remitente: Option<String>,
    pub google_email: Option<String>,
    /// "activa" o "requiere_reconexion"
    pub estado: Option<String>,
    pub sincronizando: bool,
    pub ultima_sync_fin: Option<DateTime<Utc>>,
    pub ultima_sync_ok: Option<bool>,
    /// Código corto del último error (p. ej. "token_revocado")
    pub ultima_sync_error: Option<String>,
    pub ultima_sync_importados: i32,
    pub ultima_sync_duplicados: i32,
    pub ultima_sync_fallidos: i32,
}

pub async fn status(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<GmailStatus>> {
    let Some(google) = state.google.as_ref() else {
        return Ok(Json(GmailStatus {
            disponible: false,
            conectado: false,
            remitente: None,
            google_email: None,
            estado: None,
            sincronizando: false,
            ultima_sync_fin: None,
            ultima_sync_ok: None,
            ultima_sync_error: None,
            ultima_sync_importados: 0,
            ultima_sync_duplicados: 0,
            ultima_sync_fallidos: 0,
        }));
    };

    let account = db::google_accounts::find_by_user(&state.db_pool, &auth_user.email).await?;
    let sincronizando = google.is_syncing(&auth_user.email);

    Ok(Json(match account {
        None => GmailStatus {
            disponible: true,
            conectado: false,
            remitente: Some(google.sender.clone()),
            google_email: None,
            estado: None,
            sincronizando,
            ultima_sync_fin: None,
            ultima_sync_ok: None,
            ultima_sync_error: None,
            ultima_sync_importados: 0,
            ultima_sync_duplicados: 0,
            ultima_sync_fallidos: 0,
        },
        Some(account) => GmailStatus {
            disponible: true,
            conectado: true,
            remitente: Some(google.sender.clone()),
            google_email: Some(account.google_email),
            estado: Some(account.estado),
            sincronizando,
            ultima_sync_fin: account.ultima_sync_fin,
            ultima_sync_ok: account.ultima_sync_ok,
            ultima_sync_error: account.ultima_sync_error,
            ultima_sync_importados: account.ultima_sync_importados,
            ultima_sync_duplicados: account.ultima_sync_duplicados,
            ultima_sync_fallidos: account.ultima_sync_fallidos,
        },
    }))
}

#[derive(Serialize)]
pub struct SyncStarted {
    pub iniciada: bool,
    /// Si no se inició: "ya_en_curso"
    pub motivo: Option<&'static str>,
}

pub async fn sync_now(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<(StatusCode, Json<SyncStarted>)> {
    if auth_user.is_demo {
        return Err(AppError::DemoUserRestriction);
    }
    if state.google.is_none() {
        return Err(not_configured());
    }
    let account = db::google_accounts::find_by_user(&state.db_pool, &auth_user.email)
        .await?
        .ok_or_else(|| AppError::BadRequest("Conecta primero tu cuenta de Gmail".to_string()))?;
    if account.estado != ESTADO_ACTIVA {
        return Err(AppError::BadRequest(
            "Vuelve a conectar tu cuenta de Gmail para seguir sincronizando".to_string(),
        ));
    }

    let started = match spawn_sync(&state, &auth_user.email) {
        Ok(()) => SyncStarted {
            iniciada: true,
            motivo: None,
        },
        Err(SyncError::AlreadyRunning) => SyncStarted {
            iniciada: false,
            motivo: Some("ya_en_curso"),
        },
        Err(_) => {
            return Err(AppError::InternalError(
                "No se pudo iniciar la sincronización".into(),
            ))
        }
    };
    Ok((StatusCode::ACCEPTED, Json(started)))
}

#[derive(Serialize)]
pub struct Disconnected {
    /// `false` si Google no confirmó la revocación (el token local se borra igualmente)
    pub revocado_en_google: bool,
}

pub async fn disconnect(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<Disconnected>> {
    if auth_user.is_demo {
        return Err(AppError::DemoUserRestriction);
    }
    let google = state.google.as_ref().ok_or_else(not_configured)?;

    let mut revocado = false;
    if let Some(account) =
        db::google_accounts::find_by_user(&state.db_pool, &auth_user.email).await?
    {
        let token = google
            .cipher
            .decrypt(&account.refresh_token_cifrado, &auth_user.email)
            .ok()
            .and_then(|bytes| String::from_utf8(bytes).ok());
        if let Some(token) = token {
            revocado = google.oauth.revoke(&token).await.is_ok();
        }
        if !revocado {
            tracing::warn!("No se pudo confirmar la revocación del acceso en Google");
        }
        db::google_accounts::delete_link(&state.db_pool, &auth_user.email).await?;
    }
    Ok(Json(Disconnected {
        revocado_en_google: revocado,
    }))
}

pub fn gmail_router(state: AppState) -> Router {
    Router::new()
        .route("/connect", post(connect))
        .route("/status", get(status))
        .route("/sync", post(sync_now))
        .route("/connection", delete(disconnect))
        .with_state(state)
}
