/// Tipos de error personalizados para la aplicación
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub enum AppError {
    /// Error de base de datos
    DatabaseError(String),
    /// Usuario no encontrado
    NotFound(String),
    /// Credenciales inválidas
    Unauthorized(String),
    /// Solicitud inválida (validación fallida)
    BadRequest(String),
    /// Error interno del servidor
    InternalError(String),
}

/// Estructura de respuesta de error para JSON
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            AppError::DatabaseError(msg) => {
                tracing::error!("Database error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error en la base de datos".to_string(),
                )
            }
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg),
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg),
            AppError::InternalError(msg) => {
                tracing::error!("Internal error: {}", msg);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Error interno del servidor".to_string(),
                )
            }
        };

        let body = Json(ErrorResponse {
            error: error_message,
        });

        (status, body).into_response()
    }
}

// Conversiones desde otros tipos de error
impl From<sqlx::Error> for AppError {
    fn from(err: sqlx::Error) -> Self {
        match err {
            sqlx::Error::RowNotFound => AppError::NotFound("Usuario no encontrado".to_string()),
            _ => AppError::DatabaseError(err.to_string()),
        }
    }
}

impl From<bcrypt::BcryptError> for AppError {
    fn from(err: bcrypt::BcryptError) -> Self {
        AppError::InternalError(format!("Error de bcrypt: {}", err))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        AppError::InternalError(format!("Error de JWT: {}", err))
    }
}

pub type AppResult<T> = Result<T, AppError>;
