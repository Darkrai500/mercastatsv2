//! Ajustes de los productos de una foto de ticket ya revisada, antes de la ingesta común.
//!
//! Solo se aplican a fotos: los PDF digitales traen los nombres exactos y siguen guardándose
//! tal cual por el camino de siempre.

use std::collections::HashMap;

use sqlx::PgPool;

use crate::{error::AppResult, services::TicketProduct};

/// Letras con diacrítico y su equivalente sin él, en el mismo orden que el `translate` de SQL.
const CON_TILDE: &str = "ÁÉÍÓÚÜÑÀÈÌÒÙÇ";
const SIN_TILDE: &str = "AEIOUUNAEIOUC";

/// Normaliza los productos de una foto revisada: nombres en mayúsculas, el nombre del
/// catálogo cuando la foto perdió las tildes y una sola línea por producto.
pub async fn prepare_photo_products(
    pool: &PgPool,
    productos: Vec<TicketProduct>,
) -> AppResult<Vec<TicketProduct>> {
    let mut productos: Vec<TicketProduct> = productos
        .into_iter()
        .map(|mut p| {
            p.nombre = p
                .nombre
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .to_uppercase();
            p
        })
        .collect();

    let nombres: Vec<String> = productos.iter().map(|p| p.nombre.clone()).collect();
    let canonicos = canonical_product_names(pool, &nombres).await?;
    for producto in &mut productos {
        if let Some(canonico) = canonicos.get(&producto.nombre) {
            producto.nombre = canonico.clone();
        }
    }
    Ok(merge_duplicate_products(productos))
}

/// Para cada nombre sin tildes, el único nombre del catálogo que coincide al quitárselas.
///
/// Una foto borrosa puede leer "PINA PELADA" donde el ticket dice "PIÑA PELADA"; sin esto
/// el historial del producto quedaría partido en dos. Solo se corrigen nombres sin
/// diacríticos (la versión con pérdida) y solo si la coincidencia es única.
async fn canonical_product_names(
    pool: &PgPool,
    nombres: &[String],
) -> AppResult<HashMap<String, String>> {
    let sin_tildes: Vec<String> = nombres
        .iter()
        .filter(|n| !n.chars().any(|c| CON_TILDE.contains(c)))
        .cloned()
        .collect();
    if sin_tildes.is_empty() {
        return Ok(HashMap::new());
    }

    let filas: Vec<(String, String)> = sqlx::query_as(
        "SELECT translate(nombre, $2, $3) AS clave, nombre FROM productos \
         WHERE translate(nombre, $2, $3) = ANY($1) AND translate(nombre, $2, $3) <> nombre",
    )
    .bind(&sin_tildes)
    .bind(CON_TILDE)
    .bind(SIN_TILDE)
    .fetch_all(pool)
    .await?;

    let mut candidatos: HashMap<String, Vec<String>> = HashMap::new();
    for (clave, nombre) in filas {
        candidatos.entry(clave).or_default().push(nombre);
    }
    Ok(candidatos
        .into_iter()
        .filter_map(|(clave, nombres)| match nombres.as_slice() {
            [unico] => Some((clave, unico.clone())),
            _ => None,
        })
        .collect())
}

/// Une las líneas del mismo producto (dos pesadas de fruta, o un nombre corregido en la
/// revisión que coincide con otro): la base de datos guarda una fila por producto y compra.
fn merge_duplicate_products(productos: Vec<TicketProduct>) -> Vec<TicketProduct> {
    fn cents(value: f64) -> f64 {
        (value * 100.0).round() / 100.0
    }

    let mut merged: Vec<TicketProduct> = Vec::with_capacity(productos.len());
    for producto in productos {
        match merged.iter_mut().find(|m| m.nombre == producto.nombre) {
            Some(existing) => {
                existing.cantidad =
                    ((existing.cantidad + producto.cantidad) * 1000.0).round() / 1000.0;
                existing.precio_total = cents(existing.precio_total + producto.precio_total);
                existing.descuento = cents(existing.descuento + producto.descuento);
                existing.iva_importe = cents(existing.iva_importe + producto.iva_importe);
                existing.precio_unitario =
                    cents((existing.precio_total + existing.descuento) / existing.cantidad);
            }
            None => merged.push(producto),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(
        nombre: &str,
        cantidad: f64,
        precio_unitario: f64,
        precio_total: f64,
    ) -> TicketProduct {
        TicketProduct {
            nombre: nombre.into(),
            cantidad,
            unidad: "unidad".into(),
            precio_unitario,
            precio_total,
            descuento: 0.0,
            iva_porcentaje: 4.0,
            iva_importe: 0.0,
        }
    }

    async fn seed_product(pool: &PgPool, nombre: &str) {
        sqlx::query("INSERT INTO productos (nombre) VALUES ($1)")
            .bind(nombre)
            .execute(pool)
            .await
            .unwrap();
    }

    #[test]
    fn une_las_lineas_del_mismo_producto() {
        let merged = merge_duplicate_products(vec![
            product("PLATANO", 0.912, 2.0, 1.82),
            product("LECHE", 1.0, 0.95, 0.95),
            product("PLATANO", 0.5, 2.0, 1.0),
        ]);

        assert_eq!(merged.len(), 2);
        let platano = &merged[0];
        assert_eq!(
            (
                platano.cantidad,
                platano.precio_unitario,
                platano.precio_total
            ),
            (1.412, 2.0, 2.82)
        );
    }

    #[sqlx::test(migrations = "./migrations")]
    async fn recupera_las_tildes_del_catalogo(pool: PgPool) {
        seed_product(&pool, "PIÑA PELADA RODAJAS").await;
        seed_product(&pool, "GEL WC C/ LEJÍA").await;
        // Ambiguo: dos productos distintos que se escriben igual sin tildes
        seed_product(&pool, "PAPÁ").await;
        seed_product(&pool, "PÁPA").await;

        let productos = prepare_photo_products(
            &pool,
            vec![
                product("pina  pelada rodajas", 1.0, 3.78, 3.78),
                product("GEL WC C/ LEJIA", 1.0, 0.95, 0.95),
                product("PAPA", 1.0, 1.0, 1.0),
                product("BROCOLI", 1.0, 1.59, 1.59),
                product("PIÑA PELADA RODAJAS", 1.0, 3.78, 3.78),
            ],
        )
        .await
        .unwrap();

        let nombres: Vec<&str> = productos.iter().map(|p| p.nombre.as_str()).collect();
        // La primera línea recupera la Ñ y se une a la quinta, que ya la traía
        assert_eq!(
            nombres,
            ["PIÑA PELADA RODAJAS", "GEL WC C/ LEJÍA", "PAPA", "BROCOLI"]
        );
        assert_eq!(productos[0].cantidad, 2.0);
        assert_eq!(productos[0].precio_total, 7.56);
    }
}
