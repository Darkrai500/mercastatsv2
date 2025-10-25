use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header::AUTHORIZATION, request::Parts},
};

use crate::{error::AppError, routes::auth::AppState, services::verify_jwt};

/// Usuario autenticado extraido desde el token JWT
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub email: String,
}

#[async_trait]
impl FromRequestParts<AppState> for AuthenticatedUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header_value = parts
            .headers
            .get(AUTHORIZATION)
            .ok_or_else(|| AppError::Unauthorized("Falta cabecera Authorization".to_string()))?;

        let header_str = header_value
            .to_str()
            .map_err(|_| AppError::Unauthorized("Cabecera Authorization invalida".to_string()))?;

        let token = header_str
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| AppError::Unauthorized("Formato de token invalido".to_string()))?;

        match verify_jwt(token, &state.config.jwt_secret) {
            Ok(claims) => Ok(AuthenticatedUser { email: claims.sub }),
            Err(err) => {
                tracing::warn!("Intento de acceso con JWT invalido: {:?}", err);
                Err(AppError::Unauthorized(
                    "Token invalido o expirado".to_string(),
                ))
            }
        }
    }
}
