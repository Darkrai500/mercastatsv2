use axum::{
    extract::State,
    routing::get,
    Json, Router,
};
use crate::{
    error::AppResult,
    middleware::AuthenticatedUser,
    services::{intelligence::IntelligenceService, intelligence_client::PredictionResponse},
};
use super::auth::AppState;
use crate::error::AppError;

/// Handler to get next shop prediction
pub async fn get_next_prediction(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<PredictionResponse>> {
    let service = IntelligenceService::new(state.db_pool.clone(), state.intelligence_client.clone());
    
    // Use email as user_id since we don't have a separate UUID
    let prediction = service
        .get_next_shop_prediction(auth_user.email.clone(), auth_user.email)
        .await
        .map_err(|e| AppError::InternalError(e.to_string()))?;

    Ok(Json(prediction))
}

pub fn intelligence_router(state: AppState) -> Router {
    Router::new()
        .route("/next", get(get_next_prediction))
        .with_state(state)
}
