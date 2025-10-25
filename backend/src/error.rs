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
    /// Número de factura faltante en el ticket
    MissingInvoiceNumber,
    /// Los totales del ticket no son válidos o no coinciden
    InvalidTotals(String),
    /// La compra ya existe en la base de datos (duplicado)
    DuplicatePurchase(String),
    /// Error de integridad de base de datos (constraint violation)
    DatabaseIntegrity(String),
    /// Datos del ticket inválidos
    InvalidTicketData(String),
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
            AppError::MissingInvoiceNumber => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "El ticket no contiene número de factura".to_string(),
            ),
            AppError::InvalidTotals(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
            AppError::DuplicatePurchase(invoice) => (
                StatusCode::CONFLICT,
                format!("La compra con número de factura {} ya existe", invoice),
            ),
            AppError::DatabaseIntegrity(msg) => {
                tracing::error!("Database integrity error: {}", msg);
                (
                    StatusCode::UNPROCESSABLE_ENTITY,
                    format!("Error de integridad: {}", msg),
                )
            }
            AppError::InvalidTicketData(msg) => (StatusCode::UNPROCESSABLE_ENTITY, msg),
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
            sqlx::Error::Database(db_err) => {
                // Manejar constraint violations específicas
                if let Some(constraint) = db_err.constraint() {
                    if constraint.contains("unique") || constraint.contains("pk") {
                        return AppError::DatabaseIntegrity(format!(
                            "Violación de unicidad: {}",
                            constraint
                        ));
                    } else if constraint.contains("fk") {
                        return AppError::DatabaseIntegrity(format!(
                            "Violación de clave foránea: {}",
                            constraint
                        ));
                    } else if constraint.contains("check") {
                        return AppError::DatabaseIntegrity(format!(
                            "Violación de constraint: {}",
                            constraint
                        ));
                    }
                }
                AppError::DatabaseError(db_err.to_string())
            }
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
