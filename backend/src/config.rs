/// Configuración de la aplicación cargada desde variables de entorno
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub jwt_secret: String,
    pub host: String,
    pub port: u16,
}

impl AppConfig {
    /// Carga la configuración desde variables de entorno
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok(); // Cargar .env si existe

        let database_url = std::env::var("DATABASE_URL")
            .map_err(|_| "DATABASE_URL no configurada".to_string())?;

        let jwt_secret = std::env::var("JWT_SECRET")
            .map_err(|_| "JWT_SECRET no configurada".to_string())?;

        let host = std::env::var("BACKEND_HOST")
            .unwrap_or_else(|_| "127.0.0.1".to_string());

        let port = std::env::var("BACKEND_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(8000);

        Ok(Self {
            database_url,
            jwt_secret,
            host,
            port,
        })
    }

    /// Retorna la dirección completa del servidor (host:port)
    pub fn server_addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
