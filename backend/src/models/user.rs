use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

/// Modelo de usuario que refleja la tabla `usuarios` en PostgreSQL
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub email: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub nombre: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

/// User sin el password_hash para respuestas públicas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPublic {
    pub email: String,
    pub nombre: Option<String>,
}

impl From<User> for UserPublic {
    fn from(user: User) -> Self {
        Self {
            email: user.email,
            nombre: user.nombre,
        }
    }
}
