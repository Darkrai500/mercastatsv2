use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use super::auth::AppState;
use crate::{
    db::{get_user_stats, get_user_ticket_history, TicketHistoryItem, UserStats},
    error::AppResult,
};

#[derive(Debug, Deserialize)]
pub struct HistoryQueryParams {
    usuario_email: String,
    #[serde(default)]
    limit: Option<i64>,
    #[serde(default)]
    offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct TicketHistoryResponse {
    pub tickets: Vec<TicketHistoryItem>,
    pub stats: UserStats,
}

/// Handler para obtener el histórico de tickets de un usuario
pub async fn get_user_tickets(
    State(state): State<AppState>,
    Query(params): Query<HistoryQueryParams>,
) -> AppResult<Json<TicketHistoryResponse>> {
    tracing::info!(
        "📋 Obteniendo histórico de tickets para usuario: {}",
        params.usuario_email
    );

    // Obtener tickets
    let tickets = get_user_ticket_history(
        &state.db_pool,
        &params.usuario_email,
        params.limit,
        params.offset,
    )
    .await?;

    // Obtener estadísticas
    let stats = get_user_stats(&state.db_pool, &params.usuario_email).await?;

    tracing::info!(
        "✅ Histórico obtenido: {} tickets encontrados",
        tickets.len()
    );

    Ok(Json(TicketHistoryResponse { tickets, stats }))
}

/// Router para los endpoints de tickets
pub fn tickets_router(state: AppState) -> Router {
    Router::new()
        .route("/history", get(get_user_tickets))
        .with_state(state)
}
