use chrono::{Datelike, Timelike, Utc};
use sqlx::PgPool;

use crate::{
    db::stats::get_top_products_by_quantity,
    services::intelligence_client::{
        IntelligenceClient, PredictRequest, PredictionResponse, SuggestedProduct, TicketFeature,
    },
};

#[derive(Clone)]
pub struct IntelligenceService {
    pool: PgPool,
    client: IntelligenceClient,
}

impl IntelligenceService {
    pub fn new(pool: PgPool, client: IntelligenceClient) -> Self {
        Self { pool, client }
    }

    pub async fn get_next_shop_prediction(
        &self,
        user_id: String,
        user_email: String,
    ) -> Result<PredictionResponse, anyhow::Error> {
        // 1. Fetch history features from DB View
        // Note: We cast numeric/bigint types to match Rust types
        let history: Vec<TicketFeature> = sqlx::query_as!(
            TicketFeature,
            r#"
            SELECT 
                numero_factura,
                to_char(fecha_hora, 'YYYY-MM-DD"T"HH24:MI:SS') as "fecha_hora?",
                total::float8 as "total?",
                COALESCE(day_of_week::int4, 0) as "day_of_week!",
                COALESCE(day_of_month::int4, 1) as "day_of_month!",
                COALESCE(hour_of_day::int4, 12) as "hour_of_day!",
                COALESCE(days_since_last_shop::float8, 0.0) as "days_since_last_shop!",
                COALESCE(total_last_30d::float8, 0.0) as "total_last_30d!",
                COALESCE(tickets_last_30d, 0) as "tickets_last_30d!",
                COALESCE(is_payday_week, false) as "is_payday_week!"
            FROM ml_ticket_features
            WHERE usuario_email = $1
            ORDER BY fecha_hora DESC
            LIMIT 50
            "#,
            user_email
        )
        .fetch_all(&self.pool)
        .await?;

        // 2. Calculate "Current" state features
        let now = Utc::now();

        // Calculate days since last shop based on the most recent history item
        let days_since = if let Some(last) = history.first() {
            if let Some(last_date_str) = &last.fecha_hora {
                if let Ok(last_date) =
                    chrono::NaiveDateTime::parse_from_str(last_date_str, "%Y-%m-%dT%H:%M:%S")
                {
                    let now_naive = now.naive_utc();
                    let duration = now_naive - last_date;
                    duration.num_seconds() as f64 / 86400.0
                } else {
                    0.0
                }
            } else {
                0.0
            }
        } else {
            0.0 // No history
        };

        // For total_last_30d and tickets_last_30d, we should ideally query the DB for "current" status
        // But for MVP, we can take the values from the last ticket if it was recent, or just 0.
        // Let's do a quick query to get current 30d stats if possible, or just use 0 for now to keep it simple.
        // Or better: use the values from the last ticket, as they represent the accumulation UP TO that point.
        // But that's not quite right for "now".
        // Let's assume 0 for now or reuse last ticket's values if very recent.
        let (total_30d, tickets_30d) = if let Some(last) = history.first() {
            (last.total_last_30d, last.tickets_last_30d)
        } else {
            (0.0, 0)
        };

        let features_now = TicketFeature {
            numero_factura: None,
            fecha_hora: Some(now.to_rfc3339()),
            total: None,
            day_of_week: now.weekday().num_days_from_monday() as i32, // 0 (Mon) - 6 (Sun)
            day_of_month: now.day() as i32,
            hour_of_day: now.hour() as i32,
            days_since_last_shop: days_since,
            total_last_30d: total_30d,
            tickets_last_30d: tickets_30d,
            is_payday_week: now.day() <= 7,
        };

        // 3. Call Python Service
        let req = PredictRequest {
            user_id: user_id,
            current_date: now.to_rfc3339(),
            features_now,
            history_features: history,
        };

        let mut response = self.client.predict_next(req).await?;

        // Reemplazar/inyectar siempre con productos reales del usuario
        let top_products = get_top_products_by_quantity(&self.pool, &user_email, 6).await?;
        tracing::info!(
            "Prediccion productos | usuario={} | top_items={}",
            user_email,
            top_products.len()
        );

        if !top_products.is_empty() {
            let max_qty = top_products
                .iter()
                .filter_map(|p| p.cantidad_total)
                .max()
                .unwrap_or(1) as f64;

            let suggestions: Vec<SuggestedProduct> = top_products
                .into_iter()
                .map(|p| {
                    let qty = p.cantidad_total.unwrap_or(1) as f64;
                    let probability = if max_qty > 0.0 { qty / max_qty } else { 0.5 };

                    // Usar precio_actual si existe, si no, usar precio_medio
                    let price = p
                        .precio_actual
                        .or(p.precio_medio)
                        .and_then(|d| d.to_string().parse::<f64>().ok())
                        .unwrap_or(0.0);

                    SuggestedProduct {
                        name: p.nombre,
                        probability,
                        price_estimation: price,
                        reason: format!(
                            "Frecuencia alta en tus tickets ({} unidades históricas)",
                            qty as i64
                        ),
                    }
                })
                .collect();

            response.prediction.suggested_products = suggestions;
        } else {
            tracing::warn!(
                "Prediccion productos | usuario={} | sin historial de productos, devolviendo lista vacía",
                user_email
            );
            response.prediction.suggested_products = Vec::new();
        }

        Ok(response)
    }
}
