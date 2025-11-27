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

        let intelligence_api_key =
            std::env::var("INTELLIGENCE_API_KEY").ok().filter(|v| !v.is_empty());

        let intelligence_timeout_secs = std::env::var("INTELLIGENCE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let intelligence_max_retries = std::env::var("INTELLIGENCE_MAX_RETRIES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2);

        Ok(Self {
            database_url,
            jwt_secret,
            host,
            port,
            intelligence_service_url,
            intelligence_api_key,
            intelligence_timeout_secs,
            intelligence_max_retries,
        })
    }

    /// Retorna la dirección completa del servidor (host:port)
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
