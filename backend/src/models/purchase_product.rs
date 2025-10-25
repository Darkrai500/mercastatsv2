use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Modelo de dominio para la relación entre compra y producto
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PurchaseProduct {
    pub compra_numero_factura: String,
    pub producto_nombre: String,
    pub cantidad: Decimal,
    pub precio_unitario: Decimal,
    pub precio_total: Decimal,
    pub descuento: Decimal,
    pub iva_porcentaje: Decimal,
    pub iva_importe: Decimal,
}

/// DTO para insertar un producto en una compra
#[derive(Debug, Clone)]
pub struct PurchaseProductInsert {
    pub producto_nombre: String,
    pub cantidad: Decimal,
    pub precio_unitario: Decimal,
    pub precio_total: Decimal,
    pub descuento: Decimal,
    pub iva_porcentaje: Decimal,
    pub iva_importe: Decimal,
}

impl PurchaseProductInsert {
    /// Valida que los precios sean coherentes (precio_total ≈ precio_unitario * cantidad - descuento)
    /// Permite una tolerancia de 0.01€ para redondeos
    pub fn validate_price_coherence(&self) -> bool {
        let expected = self.precio_unitario * self.cantidad - self.descuento;
        let diff = (self.precio_total - expected).abs();
        diff <= Decimal::new(1, 2) // 0.01
    }

    /// Normaliza el porcentaje de IVA a los valores estándar de España
    pub fn normalize_iva_percentage(iva: Decimal) -> Decimal {
        // IVA estándar en España: 0%, 4%, 10%, 21%
        if iva <= Decimal::new(2, 0) {
            Decimal::ZERO
        } else if iva <= Decimal::new(7, 0) {
            Decimal::new(4, 0)
        } else if iva <= Decimal::new(155, 1) {
            Decimal::new(10, 0)
        } else {
            Decimal::new(21, 0)
        }
    }

    /// Calcula el IVA importe si no está presente
    pub fn calculate_iva_importe(base: Decimal, percentage: Decimal) -> Decimal {
        base * percentage / Decimal::new(100, 0)
    }
}
