use crate::error::{AppError, AppResult};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Claims del JWT (información codificada en el token)
#[derive(Debug, Serialize, Deserialize)]
pub struct JwtClaims {
    pub sub: String, // email del usuario
    pub exp: usize,  // timestamp de expiración
    pub iat: usize,  // timestamp de emisión
}

/// Hash una contraseña con bcrypt
pub fn hash_password(password: &str) -> AppResult<String> {
    let hashed = bcrypt::hash(password, 12)?;
    Ok(hashed)
}

/// Verifica si una contraseña coincide con su hash
pub fn verify_password(password: &str, hash: &str) -> AppResult<bool> {
    let valid = bcrypt::verify(password, hash)?;
    Ok(valid)
}

/// Genera un JWT para un usuario
pub fn generate_jwt(email: &str, jwt_secret: &str) -> AppResult<String> {
    let now = Utc::now();
    let expires_in = Duration::hours(24);
    let exp = (now + expires_in).timestamp() as usize;
    let iat = now.timestamp() as usize;

    let claims = JwtClaims {
        sub: email.to_string(),
        exp,
        iat,
    };

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(jwt_secret.as_bytes()),
    )?;

    Ok(token)
}

/// Verifica y decodifica un JWT
pub fn verify_jwt(token: &str, jwt_secret: &str) -> AppResult<JwtClaims> {
    match decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::default(),
    ) {
        Ok(token_data) => Ok(token_data.claims),
        Err(err) => {
            tracing::warn!("Fallo al verificar JWT: {}", err);
            Err(AppError::Unauthorized(
                "Token inválido o expirado".to_string(),
            ))
        }
    }
}
