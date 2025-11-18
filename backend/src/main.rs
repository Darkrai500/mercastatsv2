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

/// Health check endpoint
async fn health() -> &'static str {
    "OK"
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Inicializar logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("mercastats_backend=debug".parse()?),
        )
        .init();

    // Cargar configuración
    let config = AppConfig::from_env().map_err(|e| {
        eprintln!("Error de configuración: {}", e);
        e
    })?;

    tracing::info!("Iniciando servidor en {}:{}", config.host, config.port);

    // Crear pool de conexiones a la BD
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&config.database_url)
        .await?;

    tracing::info!("Conectado a la base de datos");

    // Crear estado de la aplicación
    let state = AppState {
        db_pool: pool,
        config: config.clone(),
    };

    // Construir el router
    let app = Router::new()
        // Health check
        .route("/health", get(health))
        // Rutas de autenticación
        .nest("/api/auth", routes::auth_router(state.clone()))
        // OCR embebido
        .nest("/api/ocr", routes::ocr_router(state.clone()))
        // Rutas de tickets
        .nest("/api/tickets", routes::tickets_router(state.clone()))
        // Rutas de estadísticas
        .nest("/api/stats", routes::stats_router(state.clone()))
        // CORS middleware
        .layer(CorsLayer::permissive());

    // Crear dirección del servidor
    let addr: SocketAddr = format!("{}:{}", config.host, config.port).parse()?;
    tracing::info!("Servidor escuchando en http://{}", addr);

    // Crear listener y ejecutar
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
