use chrono::NaiveDateTime;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Modelo de dominio para una compra (ticket)
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Purchase {
    pub numero_factura: String,
    pub usuario_email: String,
    pub fecha_hora: NaiveDateTime,
    pub total: Decimal,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub metodo_pago: Option<String>,
    pub numero_operacion: Option<String>,
    pub created_at: NaiveDateTime,
}

/// DTO para insertar una compra
#[derive(Debug, Clone)]
pub struct PurchaseInsert {
    pub numero_factura: String,
    pub usuario_email: String,
    pub fecha_hora: NaiveDateTime,
    pub total: Decimal,
    pub tienda: Option<String>,
    pub ubicacion: Option<String>,
    pub metodo_pago: Option<String>,
    pub numero_operacion: Option<String>,
}

impl PurchaseInsert {
    /// Normaliza el número de factura (trim y uppercase)
    pub fn normalize_invoice_number(invoice: &str) -> String {
        invoice.trim().to_uppercase()
    }

    /// Valida que el método de pago sea uno de los permitidos
    pub fn validate_payment_method(method: &str) -> bool {
        matches!(
            method,
            "TARJETA BANCARIA" | "EFECTIVO" | "BIZUM" | "TRANSFERENCIA"
        )
    }

    /// Normaliza el método de pago si es posible, o None si no es válido
    pub fn normalize_payment_method(method: &str) -> Option<String> {
        let normalized = method.trim().to_uppercase();
        if Self::validate_payment_method(&normalized) {
            Some(normalized)
        } else {
            None
        }
    }
}
