use crate::models::{TicketPdf, TicketPdfInsert};
use sqlx::{PgPool, Postgres};

/// Inserta el PDF de un ticket en la base de datos
pub async fn insert_ticket_pdf<'c, E>(
    executor: E,
    ticket: &TicketPdfInsert,
) -> Result<TicketPdf, sqlx::Error>
where
    E: sqlx::Executor<'c, Database = Postgres>,
{
    let size_bytes = ticket.calculate_size();

    let result = sqlx::query_as!(
        TicketPdf,
        r#"
        INSERT INTO tickets_pdf (
            numero_factura,
            ticket_pdf,
            ticket_nombre_archivo,
            ticket_tamano_bytes
        )
        VALUES ($1, $2, $3, $4)
        RETURNING
            numero_factura,
            ticket_pdf,
            ticket_nombre_archivo,
            ticket_tamano_bytes,
            created_at
        "#,
        ticket.numero_factura,
        ticket.ticket_pdf,
        ticket.ticket_nombre_archivo,
        size_bytes
    )
    .fetch_one(executor)
    .await?;

    Ok(result)
}

/// Obtiene el PDF de un ticket
pub async fn get_ticket_pdf(
    pool: &PgPool,
    numero_factura: &str,
) -> Result<Option<TicketPdf>, sqlx::Error> {
    let ticket = sqlx::query_as!(
        TicketPdf,
        r#"
        SELECT
            numero_factura,
            ticket_pdf,
            ticket_nombre_archivo,
            ticket_tamano_bytes,
            created_at
        FROM tickets_pdf
        WHERE numero_factura = $1
        "#,
        numero_factura
    )
    .fetch_optional(pool)
    .await?;

    Ok(ticket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use rust_decimal::Decimal;

    #[sqlx::test]
    async fn test_insert_and_get_ticket_pdf(pool: PgPool) -> sqlx::Result<()> {
        // Setup: crear usuario y compra
        sqlx::query!(
            "INSERT INTO usuarios (email, password_hash, nombre) VALUES ($1, $2, $3)",
            "test@example.com",
            "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi",
            "Test User"
        )
        .execute(&pool)
        .await?;

        sqlx::query!(
            r#"
            INSERT INTO compras (numero_factura, usuario_email, fecha_hora, total)
            VALUES ($1, $2, $3, $4)
            "#,
            "0001-001-000001",
            "test@example.com",
            NaiveDate::from_ymd_opt(2025, 1, 15)
                .unwrap()
                .and_hms_opt(10, 30, 0)
                .unwrap(),
            Decimal::new(4565, 2)
        )
        .execute(&pool)
        .await?;

        // Test: insertar PDF
        let fake_pdf = vec![0x25, 0x50, 0x44, 0x46]; // "%PDF" magic bytes
        let ticket = TicketPdfInsert {
            numero_factura: "0001-001-000001".to_string(),
            ticket_pdf: fake_pdf.clone(),
            ticket_nombre_archivo: "ticket_test.pdf".to_string(),
        };

        let inserted = insert_ticket_pdf(&pool, &ticket).await?;

        assert_eq!(inserted.numero_factura, "0001-001-000001");
        assert_eq!(inserted.ticket_pdf, fake_pdf);
        assert_eq!(inserted.ticket_nombre_archivo, "ticket_test.pdf");
        assert_eq!(inserted.ticket_tamano_bytes, 4);

        // Test: recuperar PDF
        let retrieved = get_ticket_pdf(&pool, "0001-001-000001")
            .await?
            .expect("PDF should exist");

        assert_eq!(retrieved.ticket_pdf, fake_pdf);

        Ok(())
    }
}
