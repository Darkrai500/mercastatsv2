use chrono::Utc;
use sqlx::PgPool;

use crate::services::intelligence_client::{
    IntelligenceClient, PredictRequest, PredictionResponse, ProductPurchase, TicketFeature,
};

/// Tickets más recientes que se envían para detectar el patrón de visitas y de gasto.
const MAX_HISTORY_TICKETS: i64 = 300;
/// Tickets más recientes de los que se envían las líneas de producto (cadencia de recompra).
const MAX_PRODUCT_TICKETS: i64 = 120;

#[derive(Clone)]
pub struct IntelligenceService {
    pool: PgPool,
    client: IntelligenceClient,
}

impl IntelligenceService {
    pub fn new(pool: PgPool, client: IntelligenceClient) -> Self {
        Self { pool, client }
    }

    /// Pide al servicio de inteligencia la próxima compra del usuario.
    ///
    /// Se envían datos en bruto (instante y total de cada ticket, y líneas de producto):
    /// la ingeniería de características vive solo en el servicio Python. Las horas son
    /// las de pared del ticket (sin zona) y el instante actual va en UTC; el servicio
    /// convierte a la zona horaria de las tiendas.
    pub async fn get_next_shop_prediction(
        &self,
        user_email: &str,
    ) -> Result<PredictionResponse, anyhow::Error> {
        let history = sqlx::query_as::<_, TicketFeature>(
            r#"
            SELECT
                to_char(fecha_hora, 'YYYY-MM-DD"T"HH24:MI:SS') AS fecha_hora,
                total::float8 AS total
            FROM compras
            WHERE usuario_email = $1
            ORDER BY fecha_hora DESC
            LIMIT $2
            "#,
        )
        .bind(user_email)
        .bind(MAX_HISTORY_TICKETS)
        .fetch_all(&self.pool)
        .await?;

        let product_history = sqlx::query_as::<_, ProductPurchase>(
            r#"
            SELECT
                cp.producto_nombre AS name,
                to_char(c.fecha_hora, 'YYYY-MM-DD"T"HH24:MI:SS') AS fecha_hora,
                cp.cantidad::float8 AS quantity,
                cp.precio_unitario::float8 AS unit_price
            FROM compras c
            JOIN compras_productos cp ON cp.compra_numero_factura = c.numero_factura
            WHERE c.numero_factura IN (
                SELECT numero_factura
                FROM compras
                WHERE usuario_email = $1
                ORDER BY fecha_hora DESC
                LIMIT $2
            )
            ORDER BY c.fecha_hora DESC
            "#,
        )
        .bind(user_email)
        .bind(MAX_PRODUCT_TICKETS)
        .fetch_all(&self.pool)
        .await?;

        // Sin email ni datos de compra en los logs (ver docs/SECURITY_AUDIT.md).
        tracing::info!(
            "Solicitando prediccion | tickets={} | lineas_producto={}",
            history.len(),
            product_history.len()
        );

        let request = PredictRequest {
            current_date: Utc::now().to_rfc3339(),
            history_features: history,
            product_history,
        };

        Ok(self.client.predict_next(request).await?)
    }
}
