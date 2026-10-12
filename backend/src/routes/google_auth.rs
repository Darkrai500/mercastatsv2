//! Inicio de sesión con Google (flujo de código con PKCE) y vinculación de Gmail.
//!
//! - `GET /api/auth/providers`        qué proveedores hay activos (para mostrar el botón)
//! - `GET /api/auth/google/start`     redirige a Google (inicio de sesión)
//! - `GET /api/auth/google/callback`  vuelve de Google; crea/identifica al usuario y emite el JWT
//!
//! La vinculación desde dentro de una sesión ya iniciada (`POST /api/gmail/connect`) reutiliza
//! el mismo callback; solo cambia el modo guardado en la cookie de estado.
//!
//! Seguridad del flujo:
//! - `state` y el verificador PKCE viajan en una cookie HttpOnly firmada: atan el callback al
//!   navegador que inició el flujo (evita el "login CSRF" y que se canjee un código robado).
//! - Un inicio de sesión con Google NUNCA se enlaza solo con una cuenta de contraseña ya
//!   existente (el registro con contraseña no verifica el email: un atacante podría haberla
//!   creado antes). Para esas cuentas hay que conectar Google desde dentro de la sesión.
//! - Los errores vuelven al frontend como códigos cortos en el fragmento de la URL; nunca
//!   texto de Google ni datos del usuario.

use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, HeaderMap, StatusCode},
    response::Response,
    routing::get,
    Json,
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::auth::AppState;
use crate::{
    db,
    error::{AppError, AppResult},
    models::google_account::ESTADO_ACTIVA,
    services::{
        crypto::{constant_time_eq, pkce_challenge, random_urlsafe},
        generate_jwt,
        gmail_sync::spawn_sync,
        google::GoogleError,
        google_integration::GoogleIntegration,
    },
};

const STATE_COOKIE: &str = "mercastats_oauth";
const STATE_PURPOSE: &str = "google_oauth_state";
const STATE_TTL_SECS: i64 = 600;
const COOKIE_PATH: &str = "/api/auth/google";

// ---------------------------------------------------------------------------
// Cookie de estado
// ---------------------------------------------------------------------------

/// Contenido de la cookie de estado. Sin `Debug`: lleva el verificador PKCE.
#[derive(Serialize, Deserialize)]
pub struct OAuthStateClaims {
    purpose: String,
    pub state: String,
    verifier: String,
    /// Email del usuario al que vincular Google (modo "conectar desde la sesión")
    pub link: Option<String>,
    /// Ya se forzó una vez el consentimiento para obtener el refresh token
    pub retried: bool,
    exp: usize,
}

// Las cookies de estado y los JWT de sesión comparten secreto, pero no son intercambiables:
// la sesión exige `sub`/`iat` y el estado exige `purpose`, así que ninguno deserializa como el otro.
fn encode_state(claims: &OAuthStateClaims, secret: &str) -> AppResult<String> {
    Ok(encode(
        &Header::default(),
        claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

fn decode_state(token: &str, secret: &str) -> Option<OAuthStateClaims> {
    let data = decode::<OAuthStateClaims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .ok()?;
    (data.claims.purpose == STATE_PURPOSE).then_some(data.claims)
}

fn state_cookie(value: &str, secure: bool) -> String {
    format!(
        "{STATE_COOKIE}={value}; Path={COOKIE_PATH}; Max-Age={STATE_TTL_SECS}; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    )
}

fn clear_cookie(secure: bool) -> String {
    format!(
        "{STATE_COOKIE}=; Path={COOKIE_PATH}; Max-Age=0; HttpOnly; SameSite=Lax{}",
        if secure { "; Secure" } else { "" }
    )
}

fn cookie_value(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value.to_string())
}

/// Prepara una autorización: devuelve la URL de Google y la cookie de estado a fijar.
pub fn begin_authorization(
    state: &AppState,
    google: &GoogleIntegration,
    link: Option<String>,
    force_consent: bool,
    retried: bool,
) -> AppResult<(String, String)> {
    let internal = |_| AppError::InternalError("No se pudo generar el estado OAuth".to_string());
    let oauth_state = random_urlsafe(32).map_err(internal)?;
    let verifier = random_urlsafe(48).map_err(internal)?;

    let claims = OAuthStateClaims {
        purpose: STATE_PURPOSE.to_string(),
        state: oauth_state.clone(),
        verifier: verifier.clone(),
        link,
        retried,
        exp: (Utc::now() + Duration::seconds(STATE_TTL_SECS)).timestamp() as usize,
    };
    let cookie = state_cookie(
        &encode_state(&claims, &state.config.jwt_secret)?,
        google.secure_cookies,
    );
    let url =
        google
            .oauth
            .authorization_url(&oauth_state, &pkce_challenge(&verifier), force_consent);
    Ok((url, cookie))
}

// ---------------------------------------------------------------------------
// Respuestas
// ---------------------------------------------------------------------------

fn redirect(location: &str, set_cookie: Option<String>) -> Response {
    let mut response = Response::builder()
        .status(StatusCode::FOUND)
        .header(header::LOCATION, location)
        // El destino puede llevar el JWT en el fragmento: que no se cachee ni se filtre
        .header(header::CACHE_CONTROL, "no-store")
        .header(header::REFERRER_POLICY, "no-referrer");
    if let Some(cookie) = set_cookie {
        response = response.header(header::SET_COOKIE, cookie);
    }
    response
        .body(Body::empty())
        .expect("respuesta de redirección válida")
}

fn error_location(code: &str, mode: &str) -> String {
    format!("/auth/google#error={code}&mode={mode}")
}

/// `/auth/google#token=…&email=…&is_demo=…` (el fragmento no se envía a ningún servidor).
fn login_location(token: &str, email: &str, is_demo: bool) -> String {
    let mut url = Url::parse("http://localhost/").expect("URL base válida");
    url.query_pairs_mut()
        .append_pair("token", token)
        .append_pair("email", email)
        .append_pair("is_demo", &is_demo.to_string());
    format!("/auth/google#{}", url.query().unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct Providers {
    pub google: bool,
}

pub async fn providers(State(state): State<AppState>) -> Json<Providers> {
    Json(Providers {
        google: state.google.is_some(),
    })
}

pub async fn google_start(State(state): State<AppState>) -> Response {
    let Some(google) = state.google.as_ref() else {
        return redirect(&error_location("no_configurado", "login"), None);
    };
    match begin_authorization(&state, google, None, false, false) {
        Ok((url, cookie)) => redirect(&url, Some(cookie)),
        Err(_) => redirect(&error_location("error_interno", "login"), None),
    }
}

#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

enum Success {
    LoggedIn {
        email: String,
    },
    Linked {
        email: String,
    },
    /// Google no entregó refresh token: volver a pedir el consentimiento
    Retry,
}

pub async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    let Some(google) = state.google.as_ref() else {
        return redirect(&error_location("no_configurado", "login"), None);
    };
    let clear = Some(clear_cookie(google.secure_cookies));

    let claims = cookie_value(&headers, STATE_COOKIE)
        .and_then(|token| decode_state(&token, &state.config.jwt_secret));
    let Some(claims) = claims else {
        return redirect(&error_location("estado_invalido", "login"), clear);
    };
    let mode = if claims.link.is_some() {
        "link"
    } else {
        "login"
    };

    match complete(&state, google, &claims, params).await {
        Ok(Success::LoggedIn { email }) => {
            let is_demo = state.config.demo_user_email.as_deref() == Some(email.as_str());
            let Ok(token) = generate_jwt(&email, &state.config.jwt_secret) else {
                return redirect(&error_location("error_interno", mode), clear);
            };
            // La primera sincronización arranca ya, sin esperar al planificador
            let _ = spawn_sync(&state, &email);
            redirect(&login_location(&token, &email, is_demo), clear)
        }
        Ok(Success::Linked { email }) => {
            let _ = spawn_sync(&state, &email);
            redirect("/auth/google#linked=1&mode=link", clear)
        }
        Ok(Success::Retry) => {
            match begin_authorization(&state, google, claims.link.clone(), true, true) {
                Ok((url, cookie)) => redirect(&url, Some(cookie)),
                Err(_) => redirect(&error_location("error_interno", mode), clear),
            }
        }
        Err(code) => redirect(&error_location(code, mode), clear),
    }
}

/// Qué hacer con el usuario una vez identificado en Google.
enum Plan {
    /// Vincular Google a este usuario ya autenticado (modo "conectar")
    Link { owner: String, keep_token: bool },
    /// La cuenta de Google ya estaba vinculada: iniciar sesión como su dueño
    LoginExisting { owner: String, keep_token: bool },
    /// Cuenta sin contraseña (creada con Google) a la que se le quitó el vínculo
    Relink { owner: String },
    /// Usuario nuevo
    Create,
}

async fn complete(
    state: &AppState,
    google: &GoogleIntegration,
    claims: &OAuthStateClaims,
    params: CallbackParams,
) -> Result<Success, &'static str> {
    if params.error.is_some() {
        return Err("acceso_denegado");
    }
    let (Some(code), Some(returned_state)) = (params.code, params.state) else {
        return Err("estado_invalido");
    };
    if !constant_time_eq(&returned_state, &claims.state) {
        return Err("estado_invalido");
    }

    let tokens = google
        .oauth
        .exchange_code(&code, &claims.verifier)
        .await
        .map_err(|err| match err {
            GoogleError::InvalidGrant => "codigo_invalido",
            GoogleError::Unavailable => "google_no_disponible",
            _ => "error_google",
        })?;
    // El usuario puede desmarcar el permiso de Gmail en la pantalla de consentimiento
    if !tokens.grants_gmail() {
        return Err("permiso_gmail_denegado");
    }

    let info = google
        .oauth
        .user_info(&tokens.access_token)
        .await
        .map_err(|err| match err {
            GoogleError::Unavailable => "google_no_disponible",
            _ => "error_google",
        })?;
    let google_email = info.email.trim().to_ascii_lowercase();
    if !info.email_verified || google_email.is_empty() || info.sub.is_empty() {
        return Err("email_no_verificado");
    }

    let pool = &state.db_pool;
    let db_err = |_| "error_interno";
    let existing_by_sub = db::google_accounts::find_by_sub(pool, &info.sub)
        .await
        .map_err(db_err)?;

    let plan = if let Some(owner) = &claims.link {
        let user = db::find_user_by_email(pool, owner)
            .await
            .map_err(db_err)?
            .ok_or("cuenta_inexistente")?;
        if state.config.demo_user_email.as_deref() == Some(user.email.as_str()) {
            return Err("demo");
        }
        if existing_by_sub
            .as_ref()
            .is_some_and(|acc| &acc.usuario_email != owner)
        {
            return Err("google_en_uso");
        }
        Plan::Link {
            owner: user.email,
            keep_token: existing_by_sub
                .as_ref()
                .is_some_and(|a| a.estado == ESTADO_ACTIVA),
        }
    } else if let Some(account) = existing_by_sub {
        Plan::LoginExisting {
            keep_token: account.estado == ESTADO_ACTIVA,
            owner: account.usuario_email,
        }
    } else {
        match db::find_user_by_email(pool, &google_email)
            .await
            .map_err(db_err)?
        {
            // Ya hay una cuenta con contraseña: no se enlaza sola (ver cabecera del módulo)
            Some(user) if user.password_hash.is_some() => return Err("cuenta_existente"),
            Some(user) => {
                let linked = db::google_accounts::find_by_user(pool, &user.email)
                    .await
                    .map_err(db_err)?;
                if linked.is_some() {
                    return Err("cuenta_existente");
                }
                Plan::Relink { owner: user.email }
            }
            None => Plan::Create,
        }
    };

    let needs_token = match &plan {
        Plan::Link { keep_token, .. } | Plan::LoginExisting { keep_token, .. } => !keep_token,
        Plan::Relink { .. } | Plan::Create => true,
    };
    if needs_token && tokens.refresh_token.is_none() {
        // Google solo entrega refresh token en el primer consentimiento: se vuelve a pedir una vez
        return if claims.retried {
            Err("sin_refresh_token")
        } else {
            Ok(Success::Retry)
        };
    }

    let encrypt = |owner: &str| -> Result<Option<Vec<u8>>, &'static str> {
        tokens
            .refresh_token
            .as_ref()
            .map(|token| {
                google
                    .cipher
                    .encrypt(token.as_bytes(), owner)
                    .map_err(|_| "error_interno")
            })
            .transpose()
    };

    match plan {
        Plan::Link { owner, .. } => {
            if let Some(cipher_text) = encrypt(&owner)? {
                db::google_accounts::upsert_link(
                    pool,
                    &owner,
                    &info.sub,
                    &google_email,
                    &cipher_text,
                )
                .await
                .map_err(db_err)?;
            }
            Ok(Success::Linked { email: owner })
        }
        Plan::LoginExisting { owner, .. } => {
            if let Some(cipher_text) = encrypt(&owner)? {
                db::google_accounts::update_refresh_token(pool, &owner, &cipher_text)
                    .await
                    .map_err(db_err)?;
            }
            Ok(Success::LoggedIn { email: owner })
        }
        Plan::Relink { owner } => {
            let cipher_text = encrypt(&owner)?.ok_or("sin_refresh_token")?;
            db::google_accounts::upsert_link(pool, &owner, &info.sub, &google_email, &cipher_text)
                .await
                .map_err(db_err)?;
            Ok(Success::LoggedIn { email: owner })
        }
        Plan::Create => {
            let cipher_text = encrypt(&google_email)?.ok_or("sin_refresh_token")?;
            db::google_accounts::create_user_with_link(
                pool,
                &google_email,
                info.name.as_deref(),
                &info.sub,
                &cipher_text,
            )
            .await
            .map_err(db_err)?;
            Ok(Success::LoggedIn {
                email: google_email,
            })
        }
    }
}

pub fn google_routes() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/providers", get(providers))
        .route("/google/start", get(google_start))
        .route("/google/callback", get(google_callback))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::{generate_jwt, verify_jwt};

    const SECRET: &str = "secreto-de-prueba-con-longitud-suficiente";

    fn claims(exp_offset: i64) -> OAuthStateClaims {
        OAuthStateClaims {
            purpose: STATE_PURPOSE.into(),
            state: "estado".into(),
            verifier: "verificador".into(),
            link: Some("ana@example.com".into()),
            retried: false,
            exp: (Utc::now() + Duration::seconds(exp_offset)).timestamp() as usize,
        }
    }

    #[test]
    fn la_cookie_de_estado_ida_y_vuelta() {
        let token = encode_state(&claims(300), SECRET).unwrap();
        let back = decode_state(&token, SECRET).expect("válida");
        assert_eq!(back.state, "estado");
        assert_eq!(back.verifier, "verificador");
        assert_eq!(back.link.as_deref(), Some("ana@example.com"));
    }

    #[test]
    fn rechaza_estado_caducado_manipulado_o_con_otro_secreto() {
        let caducado = encode_state(&claims(-3600), SECRET).unwrap();
        assert!(decode_state(&caducado, SECRET).is_none());

        let bueno = encode_state(&claims(300), SECRET).unwrap();
        assert!(decode_state(&bueno, "otro-secreto").is_none());

        let mut manipulado = bueno.clone();
        manipulado.push('x');
        assert!(decode_state(&manipulado, SECRET).is_none());
        assert!(decode_state("basura", SECRET).is_none());
    }

    #[test]
    fn un_jwt_de_sesion_no_vale_como_estado_y_viceversa() {
        let sesion = generate_jwt("ana@example.com", SECRET).unwrap();
        assert!(decode_state(&sesion, SECRET).is_none());

        let estado = encode_state(&claims(300), SECRET).unwrap();
        assert!(verify_jwt(&estado, SECRET).is_err());
    }

    #[test]
    fn el_proposito_tiene_que_coincidir() {
        let mut otro = claims(300);
        otro.purpose = "otra_cosa".into();
        let token = encode_state(&otro, SECRET).unwrap();
        assert!(decode_state(&token, SECRET).is_none());
    }

    #[test]
    fn la_cookie_es_httponly_samesite_y_secure_solo_con_https() {
        let http = state_cookie("v", false);
        assert!(http.contains("HttpOnly") && http.contains("SameSite=Lax"));
        assert!(!http.contains("Secure"));
        assert!(state_cookie("v", true).contains("; Secure"));
        assert!(clear_cookie(false).contains("Max-Age=0"));
    }

    #[test]
    fn lee_la_cookie_entre_varias() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            "a=1; mercastats_oauth=valor.token; b=2".parse().unwrap(),
        );
        assert_eq!(
            cookie_value(&headers, STATE_COOKIE).as_deref(),
            Some("valor.token")
        );
        assert_eq!(cookie_value(&headers, "c"), None);
        assert_eq!(cookie_value(&HeaderMap::new(), STATE_COOKIE), None);
    }

    #[test]
    fn la_url_de_login_codifica_los_valores() {
        let loc = login_location("aaa.bbb.ccc", "ana+tickets@example.com", false);
        assert!(loc.starts_with("/auth/google#token=aaa.bbb.ccc&email="));
        assert!(loc.contains("ana%2Btickets%40example.com"));
        assert!(loc.ends_with("&is_demo=false"));
    }
}
