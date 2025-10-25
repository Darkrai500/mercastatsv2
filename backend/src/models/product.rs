use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Modelo de dominio para un producto del catálogo
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Product {
    pub nombre: String,
    pub marca: Option<String>,
    pub unidad: Option<String>,
    pub precio_actual: Option<Decimal>,
    pub precio_actualizado_en: Option<NaiveDateTime>,
    pub created_at: NaiveDateTime,
}

/// DTO para insertar o actualizar un producto
#[derive(Debug, Clone)]
pub struct ProductUpsert {
    pub nombre: String,
    pub marca: Option<String>,
    pub unidad: String,
    pub precio_actual: Option<Decimal>,
}

impl ProductUpsert {
    /// Normaliza el nombre del producto (trim y uppercase)
    pub fn normalize_name(name: &str) -> String {
        name.trim().to_uppercase()
    }

    /// Valida que la unidad sea una de las permitidas
    pub fn validate_unit(unit: &str) -> bool {
        matches!(unit, "unidad" | "kg" | "g" | "l" | "ml")
    }
}
