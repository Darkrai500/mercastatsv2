use crate::db::{DailySpendPoint, MonthlySpendPoint, TimeDistributionPoint, TopProductItem};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Respuesta del dashboard de estadísticas principal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStatsResponse {
    /// Gasto del mes actual
    pub current_month_spend: Decimal,

    /// Gasto del mes anterior
    pub previous_month_spend: Decimal,

    /// Porcentaje de tendencia (positivo o negativo)
    pub trend_percentage: f64,

    /// Total de tickets de todos los tiempos
    pub total_tickets: Option<i64>,

    /// Gasto medio por ticket
    pub average_spending_per_ticket: Option<Decimal>,

    /// Productos únicos comprados
    pub unique_products: Option<i64>,

    /// Datos de tendencia diaria (últimos 30 días)
    pub daily_spending_trend: Vec<DailySpendPoint>,

    /// Top 5 productos por cantidad
    pub top_products_quantity: Vec<TopProductItem>,

    /// Top 5 productos por gasto
    pub top_products_spending: Vec<TopProductItem>,

    /// Distribución de compras por día de la semana
    pub weekly_distribution: Vec<TimeDistributionPoint>,

    /// Distribución de compras por hora del día
    pub hourly_distribution: Vec<TimeDistributionPoint>,
}

/// Serie y métricas para la evolución mensual de gasto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyEvolutionResponse {
    pub months: Vec<MonthlySpendPoint>,
    pub current_month_total: Decimal,
    pub previous_month_total: Decimal,
    pub average_monthly: Decimal,
    pub year_to_date_total: Decimal,
    pub month_over_month: f64,
}

