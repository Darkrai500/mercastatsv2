use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::auth::AppState;
use crate::{
    db::{get_user_stats, get_user_ticket_history, TicketHistoryItem, UserStats},
    error::{AppError, AppResult},
    middleware::AuthenticatedUser,
};

#[derive(Debug, Deserialize)]
pub struct HistoryQueryParams {
    #[serde(default)]
    pub usuario_email: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
    #[serde(default)]
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TicketHistoryResponse {
    pub tickets: Vec<TicketHistoryItem>,
    pub stats: UserStats,
}

/// Handler para obtener el historico de tickets de un usuario
pub async fn get_user_tickets(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(params): Query<HistoryQueryParams>,
) -> AppResult<Json<TicketHistoryResponse>> {
    let user_email = auth_user.email;

    if let Some(ref requested_email) = params.usuario_email {
        if requested_email != &user_email {
            return Err(AppError::Unauthorized(
                "No tienes permiso para acceder a este recurso".to_string(),
            ));
        }
    }

    tracing::info!(
        "Obteniendo historico de tickets para usuario: {}",
        params.usuario_email.as_deref().unwrap_or(&user_email)
    );

    let tickets =
        get_user_ticket_history(&state.db_pool, &user_email, params.limit, params.offset).await?;

    let stats = get_user_stats(&state.db_pool, &user_email).await?;

    tracing::info!("Historico obtenido: {} tickets encontrados", tickets.len());

    Ok(Json(TicketHistoryResponse { tickets, stats }))
}

/// Router para los endpoints de tickets
pub fn tickets_router(state: AppState) -> Router {
    Router::new()
        .route("/history", get(get_user_tickets))
        .with_state(state)
}
