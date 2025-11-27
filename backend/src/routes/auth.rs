use axum::{extract::State, routing::post, Json, Router};
use sqlx::PgPool;

use crate::{
    config::AppConfig,
    db,
    error::AppResult,
    schema::{AuthResponse, LoginRequest, RegisterRequest, UserInfo},
    services::{generate_jwt, hash_password, verify_password, IntelligenceClient},
};

/// Estado compartido del servidor
#[derive(Clone)]
pub struct AppState {
    pub db_pool: PgPool,
    pub config: AppConfig,
    pub intelligence_client: IntelligenceClient,
}

/// Handler para registro de usuario
pub async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> AppResult<Json<AuthResponse>> {
    // Validar email
    if req.email.is_empty() || !req.email.contains('@') {
        return Err(crate::error::AppError::BadRequest(
            "Email inválido".to_string(),
        ));
    }

    // Validar contraseña
    if req.password.len() < 8 {
        return Err(crate::error::AppError::BadRequest(
            "La contraseña debe tener al menos 8 caracteres".to_string(),
        ));
    }

    // Verificar que el usuario no exista
    if let Ok(Some(_)) = db::find_user_by_email(&state.db_pool, &req.email).await {
        return Err(crate::error::AppError::BadRequest(
            "El email ya está registrado".to_string(),
        ));
    }

    // Hash de la contraseña
    let password_hash = hash_password(&req.password)?;

    // Crear usuario en BD
    let user = db::create_user(
        &state.db_pool,
        &req.email,
        &password_hash,
        req.nombre.as_deref(),
    )
    .await?;

    // Generar JWT
    let token = generate_jwt(&user.email, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            email: user.email,
            nombre: user.nombre,
        },
    }))
}

/// Handler para login
pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> AppResult<Json<AuthResponse>> {
    // Validar inputs
    if req.email.is_empty() || req.password.is_empty() {
        return Err(crate::error::AppError::BadRequest(
            "Email y contraseña son requeridos".to_string(),
        ));
    }

    // Buscar usuario
    let user = db::find_user_by_email(&state.db_pool, &req.email)
        .await?
        .ok_or_else(|| {
            crate::error::AppError::Unauthorized("Credenciales inválidas".to_string())
        })?;

    // Verificar contraseña
    let password_valid = verify_password(&req.password, &user.password_hash)?;
    if !password_valid {
        return Err(crate::error::AppError::Unauthorized(
            "Credenciales inválidas".to_string(),
        ));
    }

    // Generar JWT
    let token = generate_jwt(&user.email, &state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            email: user.email,
            nombre: user.nombre,
        },
    }))
}

/// Router de autenticación
pub fn auth_router(state: AppState) -> Router {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .with_state(state)
}
