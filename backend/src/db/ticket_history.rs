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
        WITH compras_stats AS (
            SELECT
                COUNT(*)::bigint AS total_tickets,
                COALESCE(SUM(total), 0)::numeric AS total_gastado,
                CASE
                    WHEN COUNT(*) = 0 THEN NULL::numeric
                    ELSE ROUND(SUM(total) / COUNT(*), 2)
                END AS gasto_medio
            FROM compras
            WHERE usuario_email = $1
        ),
        productos_stats AS (
            SELECT
                COUNT(DISTINCT cp.producto_nombre)::bigint AS productos_unicos
            FROM compras c
            LEFT JOIN compras_productos cp ON c.numero_factura = cp.compra_numero_factura
            WHERE c.usuario_email = $1
        )
        SELECT
            compras_stats.total_tickets as "total_tickets?",
            compras_stats.total_gastado as "total_gastado?",
            compras_stats.gasto_medio as "gasto_medio?",
            productos_stats.productos_unicos as "productos_unicos?"
        FROM compras_stats, productos_stats
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

        // Crear productos de referencia
        for nombre in ["LECHE ENTERA", "PAN DE MOLDE"] {
            sqlx::query!(
                "INSERT INTO productos (nombre, unidad) VALUES ($1, 'unidad')",
                nombre
            )
            .execute(&pool)
            .await?;
        }

        // Crear dos compras con totales distintos
        let compras = vec![
            (
                "0001-001-999999",
                NaiveDate::from_ymd_opt(2025, 1, 15)
                    .unwrap()
                    .and_hms_opt(10, 30, 0)
                    .unwrap(),
                Decimal::new(5000, 2), // 50.00
            ),
            (
                "0001-001-999998",
                NaiveDate::from_ymd_opt(2025, 2, 10)
                    .unwrap()
                    .and_hms_opt(12, 0, 0)
                    .unwrap(),
                Decimal::new(7000, 2), // 70.00
            ),
        ];

        for (numero_factura, fecha_hora, total) in &compras {
            sqlx::query!(
                r#"
                INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
                VALUES ($1, $2, $3, $4)
                "#,
                numero_factura,
                "stats@example.com",
                fecha_hora,
                total
            )
            .execute(&pool)
            .await?;
        }

        // Asociar productos al primer ticket (para probar duplicados en el JOIN)
        for (producto, precio_total) in [
            ("LECHE ENTERA", Decimal::new(3000, 2)),
            ("PAN DE MOLDE", Decimal::new(2000, 2)),
        ] {
            let iva_porcentaje = Decimal::new(2100, 2); // 21.00
            let descuento = Decimal::ZERO;
            let iva_importe = Decimal::ZERO;
            sqlx::query!(
                r#"
                INSERT INTO compras_productos (
                    compra_numero_factura,
                    producto_nombre,
                    cantidad,
                    precio_unitario,
                    precio_total,
                    descuento,
                    iva_porcentaje,
                    iva_importe
                )
                VALUES ($1, $2, 1, $3, $4, $5, $6, $7)
                "#,
                "0001-001-999999",
                producto,
                precio_total,
                precio_total,
                descuento,
                iva_porcentaje,
                iva_importe
            )
            .execute(&pool)
            .await?;
        }

        // Test
        let stats = get_user_stats(&pool, "stats@example.com").await?;

        assert_eq!(stats.total_tickets, Some(2));
        assert_eq!(stats.total_gastado, Some(Decimal::new(12000, 2)));
        assert_eq!(stats.gasto_medio, Some(Decimal::new(6000, 2)));
        assert_eq!(stats.productos_unicos, Some(2));

        Ok(())
    }
}
