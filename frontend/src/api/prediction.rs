use super::{get_auth_token, ApiError, API_BASE_URL};
use gloo_net::http::Request;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SuggestedProduct {
    pub name: String,
    pub probability: f64,
    pub price_estimation: f64,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PredictionResult {
    pub timestamp: String,
    pub time_window_label: String,
    pub time_window_range: String,
    pub day_label: String,
    pub estimated_total: f64,
    pub estimated_total_min: f64,
    pub estimated_total_max: f64,
    pub confidence: f64,
    pub suggested_products: Vec<SuggestedProduct>,
    #[serde(default)]
    pub learning_mode: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PredictionResponse {
    pub prediction: PredictionResult,
}

pub async fn get_next_prediction() -> Result<PredictionResponse, String> {
    let token = get_auth_token().ok_or_else(|| "No hay sesion activa".to_string())?;
    let url = format!("{}/predict/next", API_BASE_URL);

    let response = Request::get(&url)
        .header("Authorization", &format!("Bearer {}", token))
        .send()
        .await
        .map_err(|e| format!("Error de conexion: {}", e))?;

    if response.ok() {
        response
            .json::<PredictionResponse>()
            .await
            .map_err(|e| format!("Error al procesar respuesta: {}", e))
    } else {
        let status = response.status();
        let error = response
            .json::<ApiError>()
            .await
            .map(|e| e.error)
            .unwrap_or_else(|_| format!("Error {}: No se pudo obtener la predicción", status));
        Err(error)
    }
}
