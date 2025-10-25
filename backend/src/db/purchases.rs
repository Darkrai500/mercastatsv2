use crate::models::{Purchase, PurchaseInsert, PurchaseProductInsert};
use sqlx::{PgPool, Postgres};

/// Busca una compra por número de factura
pub async fn get_purchase(
    pool: &PgPool,
    numero_factura: &str,
) -> Result<Option<Purchase>, sqlx::Error> {
    let purchase = sqlx::query_as!(
        Purchase,
        r#"
        SELECT
            numero_factura,
            usuario_email,
            fecha_hora,
            total,
            tienda,
            ubicacion,
            metodo_pago,
            numero_operacion,
            created_at
        FROM compras
        WHERE numero_factura = $1
        "#,
        numero_factura
    )
    .fetch_optional(pool)
    .await?;

    Ok(purchase)
}

/// Inserta una nueva compra
pub async fn insert_purchase<'c, E>(
    executor: E,
    purchase: &PurchaseInsert,
) -> Result<Purchase, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    let result = sqlx::query_as!(
        Purchase,
        r#"
        INSERT INTO compras (
            numero_factura,
            usuario_email,
            fecha_hora,
            total,
            tienda,
            ubicacion,
            metodo_pago,
            numero_operacion
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        RETURNING
            numero_factura,
            usuario_email,
            fecha_hora,
            total,
            tienda,
            ubicacion,
            metodo_pago,
            numero_operacion,
            created_at
        "#,
        purchase.numero_factura,
        purchase.usuario_email,
        purchase.fecha_hora,
        purchase.total,
        purchase.tienda,
        purchase.ubicacion,
        purchase.metodo_pago,
        purchase.numero_operacion
    )
    .fetch_one(executor)
    .await?;

    Ok(result)
}

/// Inserta múltiples productos asociados a una compra
/// NOTA: Esta función debe llamarse dentro de una transacción junto con insert_purchase
pub async fn insert_purchase_products(
    pool: &PgPool,
    numero_factura: &str,
    items: &[PurchaseProductInsert],
) -> Result<u64, sqlx::Error> {
    let mut total_inserted = 0u64;

    for item in items {
        let result = sqlx::query!(
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
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            numero_factura,
            item.producto_nombre,
            item.cantidad,
            item.precio_unitario,
            item.precio_total,
            item.descuento,
            item.iva_porcentaje,
            item.iva_importe
        )
        .execute(pool)
        .await?;

        total_inserted += result.rows_affected();
    }

    Ok(total_inserted)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    #[sqlx::test]
    async fn test_insert_purchase(pool: PgPool) -> sqlx::Result<()> {
        // Primero crear un usuario de prueba
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "test@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi", // hash de "password"
            "Test User"
        )
        .execute(&pool)
        .await?;

        let purchase = PurchaseInsert {
            numero_factura: "0001-001-000001".to_string(),
            usuario_email: "test@example.com".to_string(),
            fecha_hora: NaiveDate::from_ymd_opt(2025, 1, 15)
                .unwrap()
                .and_hms_opt(10, 30, 0)
                .unwrap(),
            total: Decimal::new(4565, 2), // 45.65
            tienda: Some("MERCADONA S.A.".to_string()),
            ubicacion: Some("CALLE TEST 123".to_string()),
            metodo_pago: Some("TARJETA BANCARIA".to_string()),
            numero_operacion: Some("OP123456".to_string()),
        };

        let inserted = insert_purchase(&pool, &purchase).await?;

        assert_eq!(inserted.numero_factura, "0001-001-000001");
        assert_eq!(inserted.usuario_email, "test@example.com");
        assert_eq!(inserted.total, Decimal::new(4565, 2));

        Ok(())
    }
}
