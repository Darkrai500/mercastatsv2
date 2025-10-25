use crate::models::{Product, ProductUpsert};
use sqlx::{PgPool, Postgres};

/// Busca un producto por su nombre (normalizado)
pub async fn get_product(pool: &PgPool, nombre: &str) -> Result<Option<Product>, sqlx::Error> {
    let product = sqlx::query_as!(
        Product,
        r#"
        SELECT
            nombre,
            marca,
            unidad,
            precio_actual,
            precio_actualizado_en,
            created_at
        FROM productos
        WHERE nombre = $1
        "#,
        nombre
    )
    .fetch_optional(pool)
    .await?;

    Ok(product)
}

/// Inserta o actualiza un producto en el catálogo.
/// - Si el producto no existe, lo crea con todos los datos proporcionados.
/// - Si el producto existe:
///   - Actualiza marca y unidad solo si estaban vacíos antes
///   - Actualiza precio_actual solo si el nuevo precio es más reciente
///
/// Puede usarse tanto con un pool como con una transacción
pub async fn upsert_product<'c, E>(
    executor: E,
    product: &ProductUpsert,
) -> Result<Product, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    let result = sqlx::query_as!(
        Product,
        r#"
        INSERT INTO productos (nombre, marca, unidad, precio_actual, precio_actualizado_en)
        VALUES ($1, $2, $3, $4, CURRENT_TIMESTAMP)
        ON CONFLICT (nombre)
        DO UPDATE SET
            marca = COALESCE(productos.marca, EXCLUDED.marca),
            unidad = COALESCE(productos.unidad, EXCLUDED.unidad),
            precio_actual = CASE
                WHEN productos.precio_actualizado_en IS NULL
                     OR productos.precio_actualizado_en < CURRENT_TIMESTAMP
                THEN EXCLUDED.precio_actual
                ELSE productos.precio_actual
            END,
            precio_actualizado_en = CASE
                WHEN productos.precio_actualizado_en IS NULL
                     OR productos.precio_actualizado_en < CURRENT_TIMESTAMP
                THEN EXCLUDED.precio_actualizado_en
                ELSE productos.precio_actualizado_en
            END
        RETURNING
            nombre,
            marca,
            unidad,
            precio_actual,
            precio_actualizado_en,
            created_at
        "#,
        product.nombre,
        product.marca,
        product.unidad,
        product.precio_actual
    )
    .fetch_one(executor)
    .await?;

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal::Decimal;

    #[sqlx::test]
    async fn test_upsert_new_product(pool: PgPool) -> sqlx::Result<()> {
        let product = ProductUpsert {
            nombre: "LECHE ENTERA".to_string(),
            marca: Some("Hacendado".to_string()),
            unidad: "l".to_string(),
            precio_actual: Some(Decimal::new(95, 2)), // 0.95
        };

        let inserted = upsert_product(&pool, &product).await?;

        assert_eq!(inserted.nombre, "LECHE ENTERA");
        assert_eq!(inserted.marca, Some("Hacendado".to_string()));
        assert_eq!(inserted.unidad, "l");
        assert_eq!(inserted.precio_actual, Some(Decimal::new(95, 2)));

        Ok(())
    }

    #[sqlx::test]
    async fn test_upsert_existing_product_preserves_marca(pool: PgPool) -> sqlx::Result<()> {
        // Insert inicial
        let product1 = ProductUpsert {
            nombre: "PAN INTEGRAL".to_string(),
            marca: Some("Hacendado".to_string()),
            unidad: "unidad".to_string(),
            precio_actual: Some(Decimal::new(150, 2)), // 1.50
        };
        upsert_product(&pool, &product1).await?;

        // Segundo insert con marca None - no debe sobrescribir
        let product2 = ProductUpsert {
            nombre: "PAN INTEGRAL".to_string(),
            marca: None,
            unidad: "unidad".to_string(),
            precio_actual: Some(Decimal::new(160, 2)), // 1.60
        };
        let updated = upsert_product(&pool, &product2).await?;

        assert_eq!(updated.marca, Some("Hacendado".to_string()));
        assert_eq!(updated.precio_actual, Some(Decimal::new(160, 2)));

        Ok(())
    }
}
