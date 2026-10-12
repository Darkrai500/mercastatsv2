//! Cliente OAuth 2.0 / OpenID Connect de Google (flujo de código con PKCE).
//!
//! Se usa el flujo de servidor, y no el botón de Google Identity Services, porque leer Gmail en
//! segundo plano exige un *refresh token*, y solo el flujo de código con `access_type=offline`
//! lo entrega. Con un único consentimiento se obtienen identidad y acceso de lectura a Gmail.

use std::time::Duration;

use reqwest::{Client, StatusCode, Url};
use serde::Deserialize;
use thiserror::Error;

use crate::config::GoogleConfig;

/// Identidad básica + lectura de Gmail. `gmail.readonly` es un ámbito "restringido" de Google
/// (no hay uno más estrecho que dé acceso a los adjuntos).
pub const SCOPES: &str = "openid email profile https://www.googleapis.com/auth/gmail.readonly";
pub const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

const AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const TOKEN_URL: &str = "https://oauth2.googleapis.com/token";
const USERINFO_URL: &str = "https://openidconnect.googleapis.com/v1/userinfo";
const REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";

/// Errores al hablar con Google. Ninguno incluye cuerpos de respuesta: pueden llevar datos.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum GoogleError {
    /// El código de autorización o el refresh token ya no valen (revocado, caducado, reusado)
    #[error("Google rechazó la concesión (invalid_grant)")]
    InvalidGrant,
    /// Google rechazó la petición por otro motivo (p. ej. credenciales del cliente erróneas)
    #[error("Google rechazó la petición ({0})")]
    Rejected(u16),
    /// Red, timeout o error 5xx: se puede reintentar más tarde
    #[error("Google no disponible")]
    Unavailable,
    #[error("respuesta de Google no válida")]
    BadResponse,
}

/// Respuesta del endpoint de tokens.
#[derive(Deserialize)]
pub struct TokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    /// Ámbitos realmente concedidos, separados por espacios (el usuario puede desmarcar alguno)
    #[serde(default)]
    pub scope: String,
}

// A mano: los tokens no deben imprimirse nunca.
impl std::fmt::Debug for TokenResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TokenResponse")
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl TokenResponse {
    pub fn grants_gmail(&self) -> bool {
        self.scope.split_whitespace().any(|s| s == GMAIL_SCOPE)
    }
}

/// Perfil devuelto por el endpoint `userinfo` de OpenID Connect.
#[derive(Deserialize)]
pub struct UserInfo {
    pub sub: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(default)]
    pub name: Option<String>,
}

impl std::fmt::Debug for UserInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UserInfo").finish_non_exhaustive()
    }
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(default)]
    error: String,
}

pub struct GoogleOAuth {
    http: Client,
    client_id: String,
    client_secret: String,
    redirect_uri: String,
    auth_url: String,
    token_url: String,
    userinfo_url: String,
    revoke_url: String,
}

impl GoogleOAuth {
    pub fn new(config: &GoogleConfig) -> Result<Self, reqwest::Error> {
        let http = Client::builder().timeout(Duration::from_secs(20)).build()?;
        Ok(Self {
            http,
            client_id: config.client_id.clone(),
            client_secret: config.client_secret.clone(),
            redirect_uri: config.redirect_uri.clone(),
            auth_url: AUTH_URL.to_string(),
            token_url: TOKEN_URL.to_string(),
            userinfo_url: USERINFO_URL.to_string(),
            revoke_url: REVOKE_URL.to_string(),
        })
    }

    /// Apunta todos los endpoints a un servidor propio (`{base}/auth`, `/token`, `/userinfo`,
    /// `/revoke`). Solo para tests con un Google simulado.
    #[cfg(test)]
    pub fn with_base_url(mut self, base: &str) -> Self {
        let base = base.trim_end_matches('/');
        self.auth_url = format!("{base}/auth");
        self.token_url = format!("{base}/token");
        self.userinfo_url = format!("{base}/userinfo");
        self.revoke_url = format!("{base}/revoke");
        self
    }

    /// URL a la que se redirige al usuario para que dé su consentimiento.
    ///
    /// `force_consent` añade `prompt=consent`, que obliga a Google a devolver un refresh token
    /// aunque la app ya tuviera permiso. No se usa siempre para no mostrar el consentimiento en
    /// cada inicio de sesión: solo cuando falta el refresh token.
    pub fn authorization_url(
        &self,
        state: &str,
        code_challenge: &str,
        force_consent: bool,
    ) -> String {
        let mut params = vec![
            ("client_id", self.client_id.as_str()),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("response_type", "code"),
            ("scope", SCOPES),
            ("state", state),
            ("code_challenge", code_challenge),
            ("code_challenge_method", "S256"),
            ("access_type", "offline"),
        ];
        if force_consent {
            params.push(("prompt", "consent"));
        }
        Url::parse_with_params(&self.auth_url, &params)
            .expect("URL de autorización válida")
            .to_string()
    }

    /// Canjea el código del callback por tokens.
    pub async fn exchange_code(
        &self,
        code: &str,
        code_verifier: &str,
    ) -> Result<TokenResponse, GoogleError> {
        let form = [
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", self.redirect_uri.as_str()),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
        ];
        self.token_request(&form).await
    }

    /// Obtiene un access token nuevo (dura ~1 h) a partir del refresh token.
    pub async fn refresh_access_token(&self, refresh_token: &str) -> Result<String, GoogleError> {
        let form = [
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
            ("client_id", self.client_id.as_str()),
            ("client_secret", self.client_secret.as_str()),
        ];
        Ok(self.token_request(&form).await?.access_token)
    }

    async fn token_request(&self, form: &[(&str, &str)]) -> Result<TokenResponse, GoogleError> {
        let response = self
            .http
            .post(&self.token_url)
            .form(form)
            .send()
            .await
            .map_err(|_| GoogleError::Unavailable)?;

        let status = response.status();
        if status.is_success() {
            return response.json().await.map_err(|_| GoogleError::BadResponse);
        }
        Err(classify_error(
            status,
            response.json::<ErrorBody>().await.ok(),
        ))
    }

    /// Datos de perfil del usuario que acaba de dar su consentimiento.
    pub async fn user_info(&self, access_token: &str) -> Result<UserInfo, GoogleError> {
        let response = self
            .http
            .get(&self.userinfo_url)
            .bearer_auth(access_token)
            .send()
            .await
            .map_err(|_| GoogleError::Unavailable)?;

        let status = response.status();
        if status.is_success() {
            return response.json().await.map_err(|_| GoogleError::BadResponse);
        }
        Err(classify_error(status, None))
    }

    /// Revoca un token (refresh o access) en Google. Es "mejor esfuerzo": si falla, el llamante
    /// borra igualmente el token local.
    pub async fn revoke(&self, token: &str) -> Result<(), GoogleError> {
        let response = self
            .http
            .post(&self.revoke_url)
            .form(&[("token", token)])
            .send()
            .await
            .map_err(|_| GoogleError::Unavailable)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(classify_error(
                status,
                response.json::<ErrorBody>().await.ok(),
            ))
        }
    }
}

fn classify_error(status: StatusCode, body: Option<ErrorBody>) -> GoogleError {
    if status.is_server_error() || status == StatusCode::TOO_MANY_REQUESTS {
        return GoogleError::Unavailable;
    }
    match body {
        Some(body) if body.error == "invalid_grant" => GoogleError::InvalidGrant,
        _ => GoogleError::Rejected(status.as_u16()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oauth() -> GoogleOAuth {
        let cfg = GoogleConfig {
            client_id: "cliente-123".into(),
            client_secret: "secreto".into(),
            redirect_uri: "http://localhost:3000/api/auth/google/callback".into(),
            token_key: [0; 32],
        };
        GoogleOAuth::new(&cfg).unwrap()
    }

    fn params(url: &str) -> std::collections::HashMap<String, String> {
        Url::parse(url)
            .unwrap()
            .query_pairs()
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect()
    }

    #[test]
    fn la_url_de_autorizacion_lleva_pkce_y_acceso_offline() {
        let url = oauth().authorization_url("estado-xyz", "desafio-abc", false);
        assert!(url.starts_with("https://accounts.google.com/o/oauth2/v2/auth?"));

        let p = params(&url);
        assert_eq!(p["client_id"], "cliente-123");
        assert_eq!(
            p["redirect_uri"],
            "http://localhost:3000/api/auth/google/callback"
        );
        assert_eq!(p["response_type"], "code");
        assert_eq!(p["state"], "estado-xyz");
        assert_eq!(p["code_challenge"], "desafio-abc");
        assert_eq!(p["code_challenge_method"], "S256");
        assert_eq!(p["access_type"], "offline");
        assert_eq!(p["scope"], SCOPES);
        assert!(
            !p.contains_key("prompt"),
            "sin consentimiento forzado por defecto"
        );
        assert!(
            !url.contains("secreto"),
            "el secreto del cliente nunca va en la URL"
        );
    }

    #[test]
    fn el_consentimiento_forzado_añade_prompt() {
        let url = oauth().authorization_url("s", "c", true);
        assert_eq!(params(&url)["prompt"], "consent");
    }

    #[test]
    fn pide_lectura_de_gmail_pero_no_escritura() {
        assert!(SCOPES.contains("gmail.readonly"));
        assert!(!SCOPES.contains("gmail.modify"));
        assert!(!SCOPES.contains("gmail.send"));
    }

    #[test]
    fn detecta_si_el_usuario_desmarco_el_permiso_de_gmail() {
        let con = TokenResponse {
            access_token: "a".into(),
            refresh_token: None,
            scope: format!("openid email {GMAIL_SCOPE}"),
        };
        let sin = TokenResponse {
            access_token: "a".into(),
            refresh_token: None,
            scope: "openid email profile".into(),
        };
        assert!(con.grants_gmail());
        assert!(!sin.grants_gmail());
    }

    #[test]
    fn clasifica_los_errores_del_endpoint_de_tokens() {
        let grant = || {
            Some(ErrorBody {
                error: "invalid_grant".into(),
            })
        };
        assert_eq!(
            classify_error(StatusCode::BAD_REQUEST, grant()),
            GoogleError::InvalidGrant
        );
        assert_eq!(
            classify_error(
                StatusCode::UNAUTHORIZED,
                Some(ErrorBody {
                    error: "invalid_client".into()
                })
            ),
            GoogleError::Rejected(401)
        );
        assert_eq!(
            classify_error(StatusCode::BAD_GATEWAY, None),
            GoogleError::Unavailable
        );
        assert_eq!(
            classify_error(StatusCode::TOO_MANY_REQUESTS, None),
            GoogleError::Unavailable
        );
    }

    #[test]
    fn los_tokens_no_salen_en_el_debug() {
        let t = TokenResponse {
            access_token: "ya29.secreto".into(),
            refresh_token: Some("1//secreto".into()),
            scope: "openid".into(),
        };
        assert!(!format!("{:?}", t).contains("secreto"));
    }
}
