mod config;
mod db;
mod error;
mod middleware;
mod models;
mod routes;
mod schema;
mod services;

use axum::{routing::get, Router};
use sqlx::postgres::PgPoolOptions;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

use config::AppConfig;
use routes::auth::AppState;
use services::IntelligenceClient;

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mercastats_backend=debug".parse()?),
        )
        .init();

    let config = AppConfig::from_env().map_err(|e| {
        eprintln!("Error de configuracion: {}", e);
        e
    })?;

    tracing::info!("Iniciando servidor en {}:{}", config.host, config.port);

    // Crear pool de conexiones a la BD
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Conectado a la base de datos");

    // Cliente HTTP para el servicio externo de inteligencia (OCR/ML)
    let intelligence_client = IntelligenceClient::new(
        config.intelligence_service_url.clone(),
        config.intelligence_api_key.clone(),
        config.intelligence_timeout_secs,
        config.intelligence_max_retries,
    )?;

    if let Err(err) = intelligence_client.health().await {
        tracing::warn!(
            "Servicio de inteligencia no disponible en el arranque: {}",
            err
        );
    } else {
        tracing::info!("Servicio de inteligencia disponible");
    }

    // Crear estado de la aplicacion
    let state = AppState {
        db_pool: pool,
        config: config.clone(),
        intelligence_client: intelligence_client.clone(),
    };

    // Construir el router
    let app = Router::new()
        .route("/health", get(health))
        .nest("/api/auth", routes::auth_router(state.clone()))
        .nest("/api/ocr", routes::ocr_router(state.clone()))
        .nest("/api/tickets", routes::tickets_router(state.clone()))
        .nest("/api/stats", routes::stats_router(state.clone()))
        .nest("/api/predict", routes::intelligence_router(state.clone()))
        .layer(CorsLayer::permissive());

    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("Servidor escuchando en http://{}", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
