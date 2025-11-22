use super::{get_auth_token, ApiError, API_BASE_URL};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

/// Punto de data para la tendencia de gasto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailySpendPoint {
    pub fecha: String,
    pub total: String,
}

/// Información de un producto en el top
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopProductItem {
    pub nombre: String,
    pub cantidad_total: Option<i64>,
    pub gasto_total: Option<String>,
    pub precio_medio: Option<String>,
}

/// Punto de distribución temporal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeDistributionPoint {
    pub tiempo: String,
    pub total: String,
    pub cantidad_tickets: i64,
}

/// Respuesta del dashboard de estadísticas completo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardStatsResponse {
    /// Gasto del mes actual
    pub current_month_spend: String,

    /// Gasto del mes anterior
    pub previous_month_spend: String,

    /// Porcentaje de tendencia (positivo o negativo)
    pub trend_percentage: f64,

    /// Total de tickets de todos los tiempos
    pub total_tickets: Option<i64>,

    /// Gasto medio por ticket
    pub average_spending_per_ticket: Option<String>,

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

/// Obtener el dashboard completo de estadísticas
pub async fn get_dashboard_stats() -> Result<DashboardStatsResponse, String> {
    let token = get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())?;
    let url = format!("{}/stats/dashboard", API_BASE_URL);

    let response = Request::get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;

    if response.ok() {
        response
            .json::<DashboardStatsResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo obtener las estadísticas", status));
        Err(error)
    }
}

/// Punto mensual de gasto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlySpendPoint {
    pub month: String,
    pub total: String,
    pub ticket_count: i64,
}

/// Respuesta para la evolución mensual de gasto
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyEvolutionResponse {
    pub months: Vec<MonthlySpendPoint>,
    pub current_month_total: String,
    pub previous_month_total: String,
    pub average_monthly: String,
    pub year_to_date_total: String,
    pub month_over_month: f64,
}

/// Obtener la evolución mensual del gasto
pub async fn get_monthly_evolution(months: u32) -> Result<MonthlyEvolutionResponse, String> {
    let token = get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())?;
    let url = format!("{}/stats/monthly?months={}", API_BASE_URL, months);

    let response = Request::get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;

    if response.ok() {
        response
            .json::<MonthlyEvolutionResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| {
                format!(
                    "Error {}: No se pudo obtener la evolucion mensual",
                    status
                )
            });
        Err(error)
    }
}

