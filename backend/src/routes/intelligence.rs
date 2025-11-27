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

/// Handler to get next shop prediction
pub async fn get_next_prediction(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<PredictionResponse>> {
    let service = IntelligenceService::new(state.db_pool.clone(), state.intelligence_client.clone());
    
    let prediction = service
        .get_next_shop_prediction(auth_user.user_id, auth_user.email)
        .await?;

    Ok(Json(prediction))
}

pub fn intelligence_router(state: AppState) -> Router {
    Router::new()
        .route("/next", get(get_next_prediction))
        .with_state(state)
}
