use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use super::auth::AppState;
use crate::{
    db::{
        get_current_year_total, get_hourly_distribution, get_month_comparison,
        get_monthly_spending, get_spending_trend, get_top_products_by_quantity,
        get_top_products_by_spending, get_user_stats, get_weekly_distribution,
    },
    error::AppResult,
    middleware::AuthenticatedUser,
    schema::{DashboardStatsResponse, MonthlyEvolutionResponse},
};

#[derive(Debug, Deserialize)]
pub struct DashboardQueryParams {
    /// Number of days to include in the trend (default: 30)
    #[serde(default = "default_days")]
    pub days: i64,

    /// Limit for top products (default: 5)
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_days() -> i64 {
    30
}

fn default_limit() -> i64 {
    5
}

#[derive(Debug, Deserialize)]
pub struct MonthlyEvolutionQueryParams {
    /// Months to retrieve (default 12, max 24)
    #[serde(default = "default_months")]
    pub months: i64,
}

fn default_months() -> i64 {
    12
}

/// Handler: main dashboard stats
pub async fn get_dashboard_stats(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(params): Query<DashboardQueryParams>,
) -> AppResult<Json<DashboardStatsResponse>> {
    let user_email = auth_user.email;

    tracing::info!(
        "Obteniendo dashboard de estadisticas para usuario: {}",
        user_email
    );

    let month_comparison = get_month_comparison(&state.db_pool, &user_email).await?;
    let user_stats = get_user_stats(&state.db_pool, &user_email).await?;
    let daily_trend = get_spending_trend(&state.db_pool, &user_email, params.days).await?;
    let top_by_qty =
        get_top_products_by_quantity(&state.db_pool, &user_email, params.limit).await?;
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

    tracing::info!("Dashboard de estadisticas obtenido exitosamente");

    Ok(Json(response))
}

/// Handler: monthly spend evolution time series
pub async fn get_monthly_evolution(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
    Query(params): Query<MonthlyEvolutionQueryParams>,
) -> AppResult<Json<MonthlyEvolutionResponse>> {
    let user_email = auth_user.email;
    let months = params.months.clamp(3, 1000) as i32;

    let months_data = get_monthly_spending(&state.db_pool, &user_email, months).await?;

    let current_total = months_data
        .last()
        .map(|m| m.total)
        .unwrap_or(Decimal::ZERO);
    let previous_total = months_data
        .iter()
        .rev()
        .nth(1)
        .map(|m| m.total)
        .unwrap_or(Decimal::ZERO);

    let total_sum = months_data
        .iter()
        .fold(Decimal::ZERO, |acc, m| acc + m.total);
    let average_monthly = if months_data.is_empty() {
        Decimal::ZERO
    } else {
        total_sum / Decimal::from(months_data.len() as u64)
    };

    let month_over_month = if previous_total > Decimal::ZERO {
        let diff = (current_total - previous_total) / previous_total * Decimal::new(100, 0);
        diff.to_string().parse::<f64>().unwrap_or(0.0)
    } else if current_total > Decimal::ZERO {
        100.0
    } else {
        0.0
    };

    let current_year = chrono::Utc::now().format("%Y").to_string();
    // Obtener el total real del año desde la BD (no depende de months_data)
    let year_to_date_total = get_current_year_total(&state.db_pool, &user_email).await?;

    let response = MonthlyEvolutionResponse {
        months: months_data,
        current_month_total: current_total,
        previous_month_total: previous_total,
        average_monthly,
        year_to_date_total,
        month_over_month,
    };

    Ok(Json(response))
}

/// Router para los endpoints de estadisticas
pub fn stats_router(state: AppState) -> Router {
    Router::new()
        .route("/dashboard", get(get_dashboard_stats))
        .route("/monthly", get(get_monthly_evolution))
        .with_state(state)
}
