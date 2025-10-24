use sqlx::PgPool;
use crate::models::User;
use crate::error::AppResult;

/// Crear un nuevo usuario en la base de datos
pub async fn create_user(
    pool: &PgPool,
    email: &str,
    password_hash: &str,
    nombre: Option<&str>,
) -> AppResult<User> {
    let user = sqlx::query_as::<_, User>(
        r#"
        INSERT INTO usuarios (email, password_hash, nombre)
        VALUES ($1, $2, $3)
        RETURNING email, password_hash, nombre, created_at, updated_at
        "#,
    )
    .bind(email)
    .bind(password_hash)
    .bind(nombre)
    .fetch_one(pool)
    .await?;

    Ok(user)
}

/// Buscar un usuario por email
pub async fn find_user_by_email(pool: &PgPool, email: &str) -> AppResult<Option<User>> {
    let user = sqlx::query_as::<_, User>(
        r#"
        SELECT email, password_hash, nombre, created_at, updated_at
        FROM usuarios
        WHERE email = $1
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(user)
}
