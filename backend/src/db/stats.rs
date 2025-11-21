// use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Punto de data para la tendencia de gasto (serie temporal)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DailySpendPoint {
    pub fecha: String,
    pub total: Decimal,
}

/// Información de un producto en el top
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TopProductItem {
    pub nombre: String,
    pub cantidad_total: Option<i64>,
    pub gasto_total: Option<Decimal>,
    pub precio_medio: Option<Decimal>,
}

/// Estadísticas de inflación personal (comparación de precios de productos favoritos)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PersonalInflationData {
    pub producto: String,
    pub precio_medio_actual: Option<Decimal>,
    pub precio_medio_anterior: Option<Decimal>,
    pub variacion_porcentaje: Option<f64>,
}

/// Serie mensual de gasto agregada
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MonthlySpendPoint {
    pub month: String,
    pub total: Decimal,
    pub ticket_count: i64,
}

/// Obtiene la tendencia de gasto diaria para los últimos N días
pub async fn get_spending_trend(
    pool: &PgPool,
    usuario_email: &str,
    days: i64,
) -> Result<Vec<DailySpendPoint>, sqlx::Error> {
    let trend = sqlx::query_as!(
        DailySpendPoint,
        r#"
        SELECT
            DATE(c.fecha_hora)::text as "fecha!",
            SUM(c.total)::numeric as "total!"
        FROM compras c
        WHERE c.usuario_email = $1
            AND c.fecha_hora >= NOW() - INTERVAL '1 day' * $2::int
        GROUP BY DATE(c.fecha_hora)
        ORDER BY DATE(c.fecha_hora) ASC
        "#,
        usuario_email,
        days as i32
    )
    .fetch_all(pool)
    .await?;

    Ok(trend)
}

/// Obtiene los productos más comprados por cantidad
pub async fn get_top_products_by_quantity(
    pool: &PgPool,
    usuario_email: &str,
    limit: i64,
) -> Result<Vec<TopProductItem>, sqlx::Error> {
    let products = sqlx::query_as!(
        TopProductItem,
        r#"
        SELECT
            p.nombre,
            SUM(cp.cantidad)::bigint as "cantidad_total?",
            SUM(cp.precio_total)::numeric as "gasto_total?",
            ROUND(AVG(cp.precio_unitario)::numeric, 2)::numeric as "precio_medio?"
        FROM compras c
        INNER JOIN compras_productos cp ON c.numero_factura = cp.compra_numero_factura
        INNER JOIN productos p ON cp.producto_nombre = p.nombre
        WHERE c.usuario_email = $1
        GROUP BY p.nombre
        ORDER BY SUM(cp.cantidad) DESC
        LIMIT $2
        "#,
        usuario_email,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(products)
}

/// Obtiene los productos con mayor gasto
pub async fn get_top_products_by_spending(
    pool: &PgPool,
    usuario_email: &str,
    limit: i64,
) -> Result<Vec<TopProductItem>, sqlx::Error> {
    let products = sqlx::query_as!(
        TopProductItem,
        r#"
        SELECT
            p.nombre,
            SUM(cp.cantidad)::bigint as "cantidad_total?",
            SUM(cp.precio_total)::numeric as "gasto_total?",
            ROUND(AVG(cp.precio_unitario)::numeric, 2)::numeric as "precio_medio?"
        FROM compras c
        INNER JOIN compras_productos cp ON c.numero_factura = cp.compra_numero_factura
        INNER JOIN productos p ON cp.producto_nombre = p.nombre
        WHERE c.usuario_email = $1
        GROUP BY p.nombre
        ORDER BY SUM(cp.precio_total) DESC
        LIMIT $2
        "#,
        usuario_email,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(products)
}

/// Serie de gasto mensual agregada (últimos `months` meses)
pub async fn get_monthly_spending(
    pool: &PgPool,
    usuario_email: &str,
    months: i64,
) -> Result<Vec<MonthlySpendPoint>, sqlx::Error> {
    let months = months.clamp(1, 24);

    let monthly = sqlx::query_as!(
        MonthlySpendPoint,
        r#"
        WITH months AS (
            SELECT DATE_TRUNC('month', CURRENT_DATE) - INTERVAL '1 month' * generate_series(0, $2::int - 1) as month_start
        )
        SELECT
            TO_CHAR(months.month_start, 'YYYY-MM') as "month!",
            COALESCE(SUM(c.total), 0)::numeric as "total!",
            COUNT(c.numero_factura)::bigint as "ticket_count!"
        FROM months
        LEFT JOIN compras c
            ON DATE_TRUNC('month', c.fecha_hora) = months.month_start
            AND c.usuario_email = $1
        GROUP BY months.month_start
        ORDER BY months.month_start
        "#,
        usuario_email,
        months
    )
    .fetch_all(pool)
    .await?;

    Ok(monthly)
}

/// Comparación de gasto mes actual vs mes anterior
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthComparisonData {
    pub current_month_spend: Decimal,
    pub previous_month_spend: Decimal,
    pub trend_percentage: f64,
    pub days_in_current_month: Option<i32>,
}

pub async fn get_month_comparison(
    pool: &PgPool,
    usuario_email: &str,
) -> Result<MonthComparisonData, sqlx::Error> {
    let result = sqlx::query!(
        r#"
        WITH current_month AS (
            SELECT
                COALESCE(SUM(total), 0)::numeric as total,
                COUNT(DISTINCT DATE(fecha_hora))::int as days_with_purchases
            FROM compras
            WHERE usuario_email = $1
                AND DATE_TRUNC('month', fecha_hora) = DATE_TRUNC('month', CURRENT_DATE)
        ),
        previous_month AS (
            SELECT
                COALESCE(SUM(total), 0)::numeric as total
            FROM compras
            WHERE usuario_email = $1
                AND DATE_TRUNC('month', fecha_hora) = DATE_TRUNC('month', CURRENT_DATE - INTERVAL '1 month')
        )
        SELECT
            current_month.total as "current_total!",
            previous_month.total as "previous_total!",
            current_month.days_with_purchases as "days_with_purchases?"
        FROM current_month, previous_month
        "#,
        usuario_email
    )
    .fetch_one(pool)
    .await?;

    let current = result.current_total;
    let previous = result.previous_total;

    let trend_percentage = if previous > Decimal::ZERO {
        let diff = (current - previous) / previous * Decimal::new(100, 0);
        // Convert to f64 by first converting to string then parsing
        diff.to_string().parse::<f64>().unwrap_or(0.0)
    } else if current > Decimal::ZERO {
        100.0
    } else {
        0.0
    };

    Ok(MonthComparisonData {
        current_month_spend: current,
        previous_month_spend: previous,
        trend_percentage,
        days_in_current_month: result.days_with_purchases,
    })
}

/// Distribución de gasto por categoría (día de la semana, hora, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TimeDistributionPoint {
    pub tiempo: String,
    pub total: Decimal,
    pub cantidad_tickets: i64,
}

/// Obtiene distribución de compras por día de la semana
pub async fn get_weekly_distribution(
    pool: &PgPool,
    usuario_email: &str,
) -> Result<Vec<TimeDistributionPoint>, sqlx::Error> {
    let distribution = sqlx::query_as!(
        TimeDistributionPoint,
        r#"
        SELECT
            TO_CHAR(fecha_hora, 'Day') as "tiempo!",
            SUM(total)::numeric as "total!",
            COUNT(*)::bigint as "cantidad_tickets!"
        FROM compras
        WHERE usuario_email = $1
        GROUP BY EXTRACT(DOW FROM fecha_hora), TO_CHAR(fecha_hora, 'Day')
        ORDER BY EXTRACT(DOW FROM fecha_hora)
        "#,
        usuario_email
    )
    .fetch_all(pool)
    .await?;

    Ok(distribution)
}

/// Obtiene distribución de compras por hora del día
pub async fn get_hourly_distribution(
    pool: &PgPool,
    usuario_email: &str,
) -> Result<Vec<TimeDistributionPoint>, sqlx::Error> {
    let distribution = sqlx::query_as!(
        TimeDistributionPoint,
        r#"
        SELECT
            CONCAT(LPAD(EXTRACT(HOUR FROM fecha_hora)::text, 2, '0'), ':00') as "tiempo!",
            SUM(total)::numeric as "total!",
            COUNT(*)::bigint as "cantidad_tickets!"
        FROM compras
        WHERE usuario_email = $1
        GROUP BY EXTRACT(HOUR FROM fecha_hora)
        ORDER BY EXTRACT(HOUR FROM fecha_hora)
        "#,
        usuario_email
    )
    .fetch_all(pool)
    .await?;

    Ok(distribution)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[sqlx::test]
    async fn test_get_spending_trend(pool: PgPool) -> sqlx::Result<()> {
        // Setup
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "trend@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi",
            "Trend User"
        )
        .execute(&pool)
        .await?;

        // Crear compras en días diferentes
        for i in 1..=5 {
            sqlx::query!(
                r#"
                INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
                VALUES ($1, $2, $3, $4)
                "#,
                format!("0001-trend-{:06}", i),
                "trend@example.com",
                NaiveDate::from_ymd_opt(2025, 1, i as u32)
                    .unwrap()
                    .and_hms_opt(10, 0, 0)
                    .unwrap(),
                Decimal::new(1000 * i, 2)
            )
            .execute(&pool)
            .await?;
        }

        // Test
        let trend = get_spending_trend(&pool, "trend@example.com", 10).await?;

        assert!(!trend.is_empty());
        assert_eq!(trend.len(), 5);

        Ok(())
    }

    #[sqlx::test]
    async fn test_get_month_comparison(pool: PgPool) -> sqlx::Result<()> {
        // Setup
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "month@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi",
            "Month User"
        )
        .execute(&pool)
        .await?;

        // Insert current month purchase
        sqlx::query!(
            r#"
            INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
            VALUES ($1, $2, $3, $4)
            "#,
            "0001-month-current",
            "month@example.com",
            NaiveDate::from_ymd_opt(2025, 1, 15)
                .unwrap()
                .and_hms_opt(10, 0, 0)
                .unwrap(),
            Decimal::new(10000, 2)
        )
        .execute(&pool)
        .await?;

        // Test
        let comparison = get_month_comparison(&pool, "month@example.com").await?;

        assert!(comparison.current_month_spend > Decimal::ZERO);

        Ok(())
    }
}
