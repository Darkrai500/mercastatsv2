use super::auth::AppState;
use crate::error::AppError;
use crate::{
    error::AppResult,
    middleware::AuthenticatedUser,
    services::{
        intelligence::IntelligenceService,
        intelligence_client::{IntelligenceClientError, PredictionResponse},
    },
};
use axum::{extract::State, routing::get, Json, Router};

/// Convierte un fallo del servicio de predicción en el error HTTP adecuado: si el servicio
/// no responde es un 503 (reintentable), no un error interno.
fn map_prediction_error(err: anyhow::Error) -> AppError {
    if err.downcast_ref::<sqlx::Error>().is_some() {
        return AppError::DatabaseError(err.to_string());
    }
    match err.downcast_ref::<IntelligenceClientError>() {
        Some(
            IntelligenceClientError::Timeout
            | IntelligenceClientError::ServiceUnavailable
            | IntelligenceClientError::Request(_),
        ) => AppError::ServiceUnavailable(err.to_string()),
        _ => AppError::InternalError(err.to_string()),
    }
}

/// Handler to get next shop prediction
pub async fn get_next_prediction(
    State(state): State<AppState>,
    auth_user: AuthenticatedUser,
) -> AppResult<Json<PredictionResponse>> {
    let service =
        IntelligenceService::new(state.db_pool.clone(), state.intelligence_client.clone());

    let prediction = service
        .get_next_shop_prediction(&auth_user.email)
        .await
        .map_err(map_prediction_error)?;

    Ok(Json(prediction))
}

pub fn intelligence_router(state: AppState) -> Router {
    Router::new()
        .route("/next", get(get_next_prediction))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;

    fn status_of(err: anyhow::Error) -> StatusCode {
        map_prediction_error(err).into_response().status()
    }

    #[test]
    fn servicio_caido_o_lento_es_503_y_no_un_error_interno() {
        assert_eq!(
            status_of(IntelligenceClientError::Timeout.into()),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            status_of(IntelligenceClientError::ServiceUnavailable.into()),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }

    #[test]
    fn fallos_de_base_de_datos_se_reportan_como_tales() {
        assert_eq!(
            status_of(sqlx::Error::RowNotFound.into()),
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert!(matches!(
            map_prediction_error(sqlx::Error::RowNotFound.into()),
            AppError::DatabaseError(_)
        ));
    }

    #[test]
    fn respuesta_inesperada_del_servicio_es_error_interno() {
        let err = IntelligenceClientError::UnexpectedStatus {
            status: reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            body: String::new(),
        };
        assert!(matches!(
            map_prediction_error(err.into()),
            AppError::InternalError(_)
        ));
    }
}
