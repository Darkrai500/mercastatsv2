//! Persistencia de la cuenta de Google vinculada y de los correos ya procesados.

use std::collections::HashMap;

use sqlx::PgPool;

use crate::error::AppResult;
use crate::models::google_account::{ESTADO_ACTIVA, ESTADO_REQUIERE_RECONEXION};
use crate::models::{GoogleAccount, MessageOutcome, SyncCounts};

const ACCOUNT_COLUMNS: &str = "usuario_email, google_email, refresh_token_cifrado, estado, \
     ultima_sync_fin, ultima_sync_ok, ultima_sync_error, \
     ultima_sync_importados, ultima_sync_duplicados, ultima_sync_fallidos";

pub async fn find_by_sub(pool: &PgPool, google_sub: &str) -> AppResult<Option<GoogleAccount>> {
    let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM cuentas_google WHERE google_sub = $1");
    Ok(sqlx::query_as::<_, GoogleAccount>(&sql)
        .bind(google_sub)
        .fetch_optional(pool)
        .await?)
}

pub async fn find_by_user(pool: &PgPool, email: &str) -> AppResult<Option<GoogleAccount>> {
    let sql = format!("SELECT {ACCOUNT_COLUMNS} FROM cuentas_google WHERE usuario_email = $1");
    Ok(sqlx::query_as::<_, GoogleAccount>(&sql)
        .bind(email)
        .fetch_optional(pool)
        .await?)
}

/// Crea o actualiza la vinculación de un usuario existente con una cuenta de Google.
/// Deja la cuenta `activa`: es lo que ocurre al (re)conectar.
pub async fn upsert_link(
    pool: &PgPool,
    email: &str,
    google_sub: &str,
    google_email: &str,
    refresh_token_cifrado: &[u8],
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO cuentas_google (usuario_email, google_sub, google_email, refresh_token_cifrado)
        VALUES ($1, $2, $3, $4)
        ON CONFLICT (usuario_email) DO UPDATE SET
            google_sub = EXCLUDED.google_sub,
            google_email = EXCLUDED.google_email,
            refresh_token_cifrado = EXCLUDED.refresh_token_cifrado,
            estado = 'activa',
            conectado_en = CURRENT_TIMESTAMP
        "#,
    )
    .bind(email)
    .bind(google_sub)
    .bind(google_email)
    .bind(refresh_token_cifrado)
    .execute(pool)
    .await?;
    Ok(())
}

/// Crea un usuario sin contraseña y su vinculación con Google en una sola transacción, para
/// que nunca quede una cuenta sin contraseña y sin vínculo.
pub async fn create_user_with_link(
    pool: &PgPool,
    email: &str,
    nombre: Option<&str>,
    google_sub: &str,
    refresh_token_cifrado: &[u8],
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, NULL, $2)")
        .bind(email)
        .bind(nombre)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO cuentas_google (usuario_email, google_sub, google_email, refresh_token_cifrado)
        VALUES ($1, $2, $1, $3)
        "#,
    )
    .bind(email)
    .bind(google_sub)
    .bind(refresh_token_cifrado)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Sustituye el refresh token (Google a veces entrega uno nuevo) y reactiva la cuenta.
pub async fn update_refresh_token(
    pool: &PgPool,
    email: &str,
    refresh_token_cifrado: &[u8],
) -> AppResult<()> {
    sqlx::query(
        "UPDATE cuentas_google SET refresh_token_cifrado = $2, estado = 'activa' WHERE usuario_email = $1",
    )
    .bind(email)
    .bind(refresh_token_cifrado)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn delete_link(pool: &PgPool, email: &str) -> AppResult<()> {
    sqlx::query("DELETE FROM cuentas_google WHERE usuario_email = $1")
        .bind(email)
        .execute(pool)
        .await?;
    Ok(())
}

/// Emails de los usuarios cuya sincronización automática debe ejecutarse.
pub async fn list_active_emails(pool: &PgPool) -> AppResult<Vec<String>> {
    Ok(sqlx::query_scalar::<_, String>(
        "SELECT usuario_email FROM cuentas_google WHERE estado = $1 ORDER BY ultima_sync_fin NULLS FIRST",
    )
    .bind(ESTADO_ACTIVA)
    .fetch_all(pool)
    .await?)
}

pub async fn mark_needs_reconnect(pool: &PgPool, email: &str) -> AppResult<()> {
    sqlx::query("UPDATE cuentas_google SET estado = $2 WHERE usuario_email = $1")
        .bind(email)
        .bind(ESTADO_REQUIERE_RECONEXION)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_sync_started(pool: &PgPool, email: &str) -> AppResult<()> {
    sqlx::query(
        "UPDATE cuentas_google SET ultima_sync_inicio = CURRENT_TIMESTAMP WHERE usuario_email = $1",
    )
    .bind(email)
    .execute(pool)
    .await?;
    Ok(())
}

/// Cierra una ejecución. `error` es un código corto (nunca texto con datos del usuario).
pub async fn mark_sync_finished(
    pool: &PgPool,
    email: &str,
    counts: SyncCounts,
    error: Option<&str>,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE cuentas_google SET
            ultima_sync_fin = CURRENT_TIMESTAMP,
            ultima_sync_ok = $2,
            ultima_sync_error = $3,
            ultima_sync_importados = $4,
            ultima_sync_duplicados = $5,
            ultima_sync_fallidos = $6
        WHERE usuario_email = $1
        "#,
    )
    .bind(email)
    .bind(error.is_none())
    .bind(error)
    .bind(counts.importados as i32)
    .bind(counts.duplicados as i32)
    .bind(counts.fallidos as i32)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mensaje de Gmail ya tratado: resultado e intentos acumulados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedMessage {
    pub resultado: String,
    pub intentos: i32,
}

pub async fn processed_messages(
    pool: &PgPool,
    email: &str,
) -> AppResult<HashMap<String, ProcessedMessage>> {
    let rows = sqlx::query_as::<_, (String, String, i32)>(
        "SELECT gmail_message_id, resultado, intentos FROM gmail_mensajes_procesados WHERE usuario_email = $1",
    )
    .bind(email)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, resultado, intentos)| {
            (
                id,
                ProcessedMessage {
                    resultado,
                    intentos,
                },
            )
        })
        .collect())
}

pub async fn record_message(
    pool: &PgPool,
    email: &str,
    message_id: &str,
    outcome: MessageOutcome,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO gmail_mensajes_procesados (usuario_email, gmail_message_id, resultado)
        VALUES ($1, $2, $3)
        ON CONFLICT (usuario_email, gmail_message_id) DO UPDATE SET
            resultado = EXCLUDED.resultado,
            intentos = gmail_mensajes_procesados.intentos + 1,
            procesado_en = CURRENT_TIMESTAMP
        "#,
    )
    .bind(email)
    .bind(message_id)
    .bind(outcome.as_str())
    .execute(pool)
    .await?;
    Ok(())
}
