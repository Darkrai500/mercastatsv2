use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use super::auth::AppState;
use crate::{
    db::{
        get_hourly_distribution, get_month_comparison, get_spending_trend,
        get_top_products_by_quantity, get_top_products_by_spending, get_user_stats,
        get_weekly_distribution,
    },
    error::AppResult,
    middleware::AuthenticatedUser,
    schema::DashboardStatsResponse,
};

#[derive(Debug, Deserialize)]
pub struct DashboardQueryParams {
    /// Número de días a incluir en la tendencia (default: 30)
    #[serde(default = "default_days")]
    pub days: i64,

    /// Número de productos top a retornar (default: 5)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_days() -> i64 {
    30
}

fn default_limit() -> i64 {
    5
}

/// Handler para obtener el dashboard completo de estadísticas
pub async fn get_dashboard_stats(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(params): Query<DashboardQueryParams>,
) -> AppResult<Json<DashboardStatsResponse>> {
    let user_email = auth_user.email;

    tracing::info!(
        "Obteniendo dashboard de estadísticas para usuario: {}",
        user_email
    );

    // Obtener datos en paralelo (simulado - se ejecutan secuencialmente pero podrían ser paralelos)
    let month_comparison = get_month_comparison(&state.db_pool, &user_email).await?;
    let user_stats = get_user_stats(&state.db_pool, &user_email).await?;
    let daily_trend = get_spending_trend(&state.db_pool, &user_email, params.days).await?;
    let top_by_qty = get_top_products_by_quantity(&state.db_pool, &user_email, params.limit).await?;
    let top_by_spending =
        get_top_products_by_spending(&state.db_pool, &user_email, params.limit).await?;
    let weekly_dist = get_weekly_distribution(&state.db_pool, &user_email).await?;
    let hourly_dist = get_hourly_distribution(&state.db_pool, &user_email).await?;

    let response = DashboardStatsResponse {
        current_month_spend: month_comparison.current_month_spend,
        previous_month_spend: month_comparison.previous_month_spend,
        trend_percentage: month_comparison.trend_percentage,
        total_tickets: user_stats.total_tickets,
        average_spending_per_ticket: user_stats.gasto_medio,
        unique_products: user_stats.productos_unicos,
        daily_spending_trend: daily_trend,
        top_products_quantity: top_by_qty,
        top_products_spending: top_by_spending,
        weekly_distribution: weekly_dist,
        hourly_distribution: hourly_dist,
    };

    tracing::info!("Dashboard de estadísticas obtenido exitosamente");

    Ok(Json(response))
}

/// Router para los endpoints de estadísticas
pub fn stats_router(state: AppState) -> Router {
    Router::new()
        .route("/dashboard", get(get_dashboard_stats))
        .with_state(state)
}
