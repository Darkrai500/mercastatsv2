use base64::{engine::general_purpose, Engine as _};

/// Remitente de los tickets digitales de Mercadona (se puede cambiar con GMAIL_TICKET_SENDER).
pub const DEFAULT_TICKET_SENDER: &str = "ticket_digital@mail.mercadona.com";

/// Credenciales de la integración con Google. Solo existe si está todo configurado.
#[derive(Clone)]
pub struct GoogleConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    /// Clave AES-256 con la que se cifran los refresh tokens en la base de datos
    pub token_key: [u8; 32],
}

// A mano para que el secreto y la clave no acaben en un `{:?}` de la configuración.
impl std::fmt::Debug for GoogleConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleConfig")
            .field("client_id", &self.client_id)
            .field("redirect_uri", &self.redirect_uri)
            .finish_non_exhaustive()
    }
}

impl GoogleConfig {
    /// `true` si el redirect es HTTPS (la cookie de estado OAuth lleva entonces `Secure`).
    pub fn is_https(&self) -> bool {
        self.redirect_uri.starts_with("https://")
    }
}

/// Construye la configuración de Google a partir de los valores crudos del entorno.
///
/// `Ok(None)` = integración desactivada (sin credenciales). Una configuración a medias es un
/// error: es preferible fallar al arrancar que descubrirlo cuando alguien pulsa el botón.
pub fn parse_google_config(
    client_id: Option<String>,
    client_secret: Option<String>,
    redirect_uri: Option<String>,
    token_key_b64: Option<String>,
) -> Result<Option<GoogleConfig>, String> {
    let (client_id, client_secret) = match (client_id, client_secret) {
        (None, None) => return Ok(None),
        (Some(id), Some(secret)) => (id, secret),
        _ => {
            return Err(
                "GOOGLE_CLIENT_ID y GOOGLE_CLIENT_SECRET deben configurarse juntos".to_string(),
            )
        }
    };

    let key_b64 = token_key_b64.ok_or_else(|| {
        "TOKEN_ENCRYPTION_KEY es obligatoria con Google (openssl rand -base64 32)".to_string()
    })?;
    let key_bytes = general_purpose::STANDARD
        .decode(key_b64.trim())
        .map_err(|_| "TOKEN_ENCRYPTION_KEY no es base64 válido".to_string())?;
    let token_key: [u8; 32] = key_bytes
        .try_into()
        .map_err(|_| "TOKEN_ENCRYPTION_KEY debe decodificar a exactamente 32 bytes".to_string())?;

    let redirect_uri = redirect_uri
        .unwrap_or_else(|| "http://localhost:3000/api/auth/google/callback".to_string());
    if !redirect_uri.starts_with("http://") && !redirect_uri.starts_with("https://") {
        return Err("GOOGLE_REDIRECT_URI debe ser una URL http(s) absoluta".to_string());
    }

    Ok(Some(GoogleConfig {
        client_id,
        client_secret,
        redirect_uri,
        token_key,
    }))
}

/// Configuración de la aplicación cargada desde variables de entorno
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
    pub intelligence_service_url: String,
    pub intelligence_api_key: Option<String>,
    pub intelligence_timeout_secs: u64,
    pub intelligence_max_retries: u32,
    pub demo_user_email: Option<String>,
    pub cors_origins: Vec<String>,
    /// Inicio de sesión con Google + lectura de Gmail (None = desactivado)
    pub google: Option<GoogleConfig>,
    /// Remitente cuyos correos se importan
    pub gmail_ticket_sender: String,
    /// Cada cuántos minutos se sincronizan las cuentas conectadas (0 = solo bajo demanda)
    pub gmail_sync_interval_minutes: u64,
}

fn env_non_empty(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

impl AppConfig {
    /// Carga la configuración desde variables de entorno
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok(); // Cargar .env si existe

        let database_url =
            std::env::var("DATABASE_URL").map_err(|_| "DATABASE_URL no configurada".to_string())?;

        let jwt_secret =
            std::env::var("JWT_SECRET").map_err(|_| "JWT_SECRET no configurada".to_string())?;

        let host = std::env::var("BACKEND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());

        let port = std::env::var("BACKEND_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8000);

        let intelligence_service_url = std::env::var("INTELLIGENCE_SERVICE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8001".to_string());

        let intelligence_api_key = std::env::var("INTELLIGENCE_API_KEY")
            .ok()
            .filter(|v| !v.is_empty());

        let intelligence_timeout_secs = std::env::var("INTELLIGENCE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let intelligence_max_retries = std::env::var("INTELLIGENCE_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        let cors_origins = std::env::var("CORS_ORIGINS")
            .unwrap_or_else(|_| "http://localhost:3000,http://localhost:8080".to_string())
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();

        if cors_origins.is_empty() {
            return Err("CORS_ORIGINS no contiene ningún origen válido".to_string());
        }

        let google = parse_google_config(
            env_non_empty("GOOGLE_CLIENT_ID"),
            env_non_empty("GOOGLE_CLIENT_SECRET"),
            env_non_empty("GOOGLE_REDIRECT_URI"),
            env_non_empty("TOKEN_ENCRYPTION_KEY"),
        )?;

        let gmail_ticket_sender = env_non_empty("GMAIL_TICKET_SENDER")
            .unwrap_or_else(|| DEFAULT_TICKET_SENDER.to_string());
        // Se interpola en la búsqueda de Gmail: solo caracteres de una dirección de correo.
        if !gmail_ticket_sender
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "@._+-".contains(c))
        {
            return Err("GMAIL_TICKET_SENDER no es una dirección de correo válida".to_string());
        }

        let gmail_sync_interval_minutes = env_non_empty("GMAIL_SYNC_INTERVAL_MINUTES")
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        Ok(Self {
            database_url,
            jwt_secret,
            host,
            port,
            intelligence_service_url,
            intelligence_api_key,
            intelligence_timeout_secs,
            intelligence_max_retries,
            demo_user_email: std::env::var("DEMO_USER_EMAIL")
                .ok()
                .filter(|v| !v.is_empty()),
            cors_origins,
            google,
            gmail_ticket_sender,
            gmail_sync_interval_minutes,
        })
    }

    /// Retorna la dirección completa del servidor (host:port)
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key() -> Option<String> {
        Some(general_purpose::STANDARD.encode([7u8; 32]))
    }

    #[test]
    fn sin_credenciales_la_integracion_queda_desactivada() {
        assert!(parse_google_config(None, None, None, None)
            .unwrap()
            .is_none());
        // Aunque haya clave, sin credenciales no se activa
        assert!(parse_google_config(None, None, None, key())
            .unwrap()
            .is_none());
    }

    #[test]
    fn credenciales_a_medias_son_un_error() {
        assert!(parse_google_config(Some("id".into()), None, None, key()).is_err());
        assert!(parse_google_config(None, Some("secret".into()), None, key()).is_err());
    }

    #[test]
    fn exige_una_clave_de_cifrado_de_32_bytes() {
        let id = || Some("id".to_string());
        let secret = || Some("secret".to_string());
        assert!(parse_google_config(id(), secret(), None, None).is_err());
        assert!(parse_google_config(id(), secret(), None, Some("no es base64!!".into())).is_err());
        let corta = Some(general_purpose::STANDARD.encode([1u8; 16]));
        assert!(parse_google_config(id(), secret(), None, corta).is_err());
    }

    #[test]
    fn configuracion_completa_usa_el_redirect_local_por_defecto() {
        let cfg = parse_google_config(Some("id".into()), Some("secret".into()), None, key())
            .unwrap()
            .unwrap();
        assert_eq!(
            cfg.redirect_uri,
            "http://localhost:3000/api/auth/google/callback"
        );
        assert!(!cfg.is_https());
        assert_eq!(cfg.token_key, [7u8; 32]);
    }

    #[test]
    fn rechaza_redirects_que_no_son_urls_absolutas() {
        let r = parse_google_config(
            Some("id".into()),
            Some("secret".into()),
            Some("/api/auth/google/callback".into()),
            key(),
        );
        assert!(r.is_err());
    }

    #[test]
    fn el_debug_no_filtra_secretos() {
        let cfg = parse_google_config(Some("id".into()), Some("super-secreto".into()), None, key())
            .unwrap()
            .unwrap();
        let texto = format!("{:?}", cfg);
        assert!(!texto.contains("super-secreto"));
        assert!(!texto.contains("token_key"));
    }
}
