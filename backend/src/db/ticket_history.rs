use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Resumen de un ticket para el histórico del usuario
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TicketHistoryItem {
    pub numero_factura: String,
    pub fecha_hora: NaiveDateTime,
    pub total: Decimal,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub num_productos: Option<i64>,
    pub created_at: NaiveDateTime,
}

/// Obtiene todos los tickets de un usuario ordenados por fecha (más recientes primero)
pub async fn get_user_ticket_history(
    pool: &PgPool,
    usuario_email: &str,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<TicketHistoryItem>, sqlx::Error> {
    let limit = limit.unwrap_or(50);
    let offset = offset.unwrap_or(0);

    let tickets = sqlx::query_as!(
        TicketHistoryItem,
        r#"
        SELECT
            c.numero_factura,
            c.fecha_hora,
            c.total,
            c.tienda,
            c.ubicacion,
            c.created_at,
            COUNT(cp.producto_nombre) as "num_productos?"
        FROM compras c
        LEFT JOIN compras_productos cp ON c.numero_factura = cp.compra_numero_factura
        WHERE c.usuario_email = $1
        GROUP BY c.numero_factura, c.fecha_hora, c.total, c.tienda, c.ubicacion, c.created_at
        ORDER BY c.fecha_hora DESC, c.created_at DESC
        LIMIT $2 OFFSET $3
        "#,
        usuario_email,
        limit,
        offset
    )
    .fetch_all(pool)
    .await?;

    Ok(tickets)
}

/// Obtiene estadísticas del usuario
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct UserStats {
    pub total_tickets: Option<i64>,
    pub total_gastado: Option<Decimal>,
    pub gasto_medio: Option<Decimal>,
    pub productos_unicos: Option<i64>,
}

pub async fn get_user_stats(pool: &PgPool, usuario_email: &str) -> Result<UserStats, sqlx::Error> {
    let stats = sqlx::query_as!(
        UserStats,
        r#"
        SELECT
            stats.total_tickets as "total_tickets?",
            stats.total_gastado as "total_gastado?",
            stats.gasto_medio as "gasto_medio?",
            COALESCE(product_stats.productos_unicos, 0) as "productos_unicos?"
        FROM (
            SELECT
                COUNT(*)::bigint AS total_tickets,
                COALESCE(SUM(total), 0)::numeric AS total_gastado,
                COALESCE(AVG(total), 0)::numeric AS gasto_medio
            FROM compras
            WHERE usuario_email = $1
        ) stats
        LEFT JOIN (
            SELECT
                COUNT(DISTINCT cp.producto_nombre)::bigint AS productos_unicos
            FROM compras_productos cp
            INNER JOIN compras c ON cp.compra_numero_factura = c.numero_factura
            WHERE c.usuario_email = $1
        ) product_stats ON TRUE
        "#,
        usuario_email
    )
    .fetch_one(pool)
    .await?;

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[sqlx::test]
    async fn test_get_user_ticket_history(pool: PgPool) -> sqlx::Result<()> {
        // Setup: crear usuario
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "test@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi",
            "Test User"
        )
        .execute(&pool)
        .await?;

        // Crear compras de prueba
        for i in 1..=3 {
            sqlx::query!(
                r#"
                INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total, tienda)
                VALUES ($1, $2, $3, $4, $5)
                "#,
                format!("0001-001-{:06}", i),
                "test@example.com",
                NaiveDate::from_ymd_opt(2025, 1, i as u32)
                    .unwrap()
                    .and_hms_opt(10, 30, 0)
                    .unwrap(),
                Decimal::new(1000 * i, 2), // 10.00, 20.00, 30.00
                "MERCADONA S.A."
            )
            .execute(&pool)
            .await?;
        }

        // Test: obtener histórico
        let history = get_user_ticket_history(&pool, "test@example.com", None, None).await?;

        assert_eq!(history.len(), 3);
        // Verificar que están ordenados por fecha descendente
        assert_eq!(history[0].numero_factura, "0001-001-000003");
        assert_eq!(history[1].numero_factura, "0001-001-000002");
        assert_eq!(history[2].numero_factura, "0001-001-000001");

        Ok(())
    }

    #[sqlx::test]
    async fn test_get_user_stats(pool: PgPool) -> sqlx::Result<()> {
        // Setup
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "stats@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi",
            "Stats User"
        )
        .execute(&pool)
        .await?;

        // Crear compra
        sqlx::query!(
            r#"
            INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
            VALUES ($1, $2, $3, $4)
            "#,
            "0001-001-999999",
            "stats@example.com",
            NaiveDate::from_ymd_opt(2025, 1, 15)
                .unwrap()
                .and_hms_opt(10, 30, 0)
                .unwrap(),
            Decimal::new(5000, 2) // 50.00
        )
        .execute(&pool)
        .await?;

        // Test
        let stats = get_user_stats(&pool, "stats@example.com").await?;

        assert_eq!(stats.total_tickets, Some(1));
        assert_eq!(stats.total_gastado, Some(Decimal::new(5000, 2)));
        assert_eq!(stats.gasto_medio, Some(Decimal::new(5000, 2)));

        Ok(())
    }

    #[sqlx::test]
    async fn test_get_user_stats_without_double_counting(pool: PgPool) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "multi@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi",
            "Multi Stats User"
        )
        .execute(&pool)
        .await?;

        // Primera compra con dos productos
        sqlx::query!(
            r#"
            INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
            VALUES ($1, $2, $3, $4)
            "#,
            "0001-001-100000",
            "multi@example.com",
            NaiveDate::from_ymd_opt(2025, 2, 10)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap(),
            Decimal::new(3000, 2) // 30.00
        )
        .execute(&pool)
        .await?;

        for (producto, precio) in [("Manzanas", 1500_i64), ("Naranjas", 1500_i64)] {
            sqlx::query!(
                r#"
                INSERT INTO compras_productos (compra_numero_factura, producto_nombre, precio_total)
                VALUES ($1, $2, $3)
                "#,
                "0001-001-100000",
                producto,
                Decimal::new(precio, 2)
            )
            .execute(&pool)
            .await?;
        }

        // Segunda compra sin productos asociados
        sqlx::query!(
            r#"
            INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
            VALUES ($1, $2, $3, $4)
            "#,
            "0001-001-100001",
            "multi@example.com",
            NaiveDate::from_ymd_opt(2025, 2, 11)
                .unwrap()
                .and_hms_opt(9, 0, 0)
                .unwrap(),
            Decimal::new(3000, 2) // 30.00
        )
        .execute(&pool)
        .await?;

        let stats = get_user_stats(&pool, "multi@example.com").await?;

        assert_eq!(stats.total_tickets, Some(2));
        assert_eq!(stats.total_gastado, Some(Decimal::new(6000, 2))); // 60.00
        assert_eq!(stats.gasto_medio, Some(Decimal::new(3000, 2))); // 30.00
        assert_eq!(stats.productos_unicos, Some(2));

        Ok(())
    }
}
