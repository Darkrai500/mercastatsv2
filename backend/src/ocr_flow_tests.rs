//! Pruebas de integración de la subida de tickets: fotos revisadas (`/api/ocr/confirm`) y el
//! camino de siempre de los PDF (`/api/ocr/process`).
//!
//! Corren contra la base de datos real (`#[sqlx::test]`) y el OCR simulado de `test_support`.
//! Datos 100 % sintéticos.

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Value};
use sqlx::PgPool;

use crate::{db, routes, routes::auth::AppState, test_support::*};

const EMAIL: &str = "foto@example.com";
const DEMO: &str = "demo@example.com";
const HASH: &str = "$2b$12$KpIEW.jQKvqXfN5nDwAXLub8RCRYjqNvCLKXfzHpFGK2FQJGmqQJi";

/// Cabecera JPEG seguida de contenido sintético: basta para que se reconozca como foto.
fn fake_photo(extra: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8, 0xFF, 0xE0];
    bytes.extend_from_slice(extra);
    bytes
}

fn reviewed_ticket(photo: &[u8]) -> Value {
    json!({
        "file_name": "IMG_0001.jpg",
        "file_content_b64": STANDARD.encode(photo),
        "numero_factura": "0000-000-000001",
        "fecha_hora": "2026-02-01T18:30:00",
        "total": 7.09,
        "tienda": "MERCADONA, S.A.",
        "ubicacion": "C/ EJEMPLO 1, 00000 CIUDAD",
        "metodo_pago": "TARJETA BANCARIA",
        "numero_operacion": "000123",
        "productos": [
            {"nombre": "LECHE ENTERA", "cantidad": 2.0, "unidad": "unidad", "precio_unitario": 0.95, "precio_total": 1.90, "iva_porcentaje": 4.0},
            {"nombre": "PAN DE MOLDE", "cantidad": 1.0, "unidad": "unidad", "precio_unitario": 1.25, "precio_total": 1.25, "iva_porcentaje": 4.0},
            {"nombre": "MANZANA", "cantidad": 0.82, "unidad": "kg", "precio_unitario": 2.0, "precio_total": 1.64, "iva_porcentaje": 4.0},
            {"nombre": "CAFÉ MOLIDO", "cantidad": 1.0, "unidad": "unidad", "precio_unitario": 2.30, "precio_total": 2.30, "iva_porcentaje": 10.0}
        ],
        "iva_desglose": [
            {"porcentaje": 4.0, "base_imponible": 4.61, "cuota": 0.18},
            {"porcentaje": 10.0, "base_imponible": 2.09, "cuota": 0.21}
        ]
    })
}

async fn setup(pool: PgPool) -> (FakeGoogle, AppState, Router) {
    let fake = FakeGoogle::start().await;
    let state = test_state(pool, &fake);
    db::create_user(&state.db_pool, EMAIL, HASH, Some("Foto Prueba"))
        .await
        .unwrap();
    let app = Router::new().nest("/api/ocr", routes::ocr_router(state.clone()));
    (fake, state, app)
}

fn post_json(uri: &str, email: &str, body: &Value) -> Request<Body> {
    let token = crate::services::generate_jwt(email, JWT_SECRET).unwrap();
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

async fn scalar(pool: &PgPool, sql: &str) -> i64 {
    sqlx::query_scalar(sql)
        .bind(EMAIL)
        .fetch_one(pool)
        .await
        .unwrap()
}

// ---------------------------------------------------------------------------
// Fotos revisadas
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn guarda_la_foto_revisada_sin_repetir_el_ocr(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let photo = fake_photo(b"foto sintetica");

    let reply = send(
        &app,
        post_json("/api/ocr/confirm", EMAIL, &reviewed_ticket(&photo)),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json());
    let body = reply.json();
    assert_eq!(body["ingested"], true);
    assert_eq!(body["numero_factura"], "0000-000-000001");
    assert_eq!(body["productos_insertados"], 4);
    assert_eq!(fake.with(|s| s.ocr_calls), 0);

    let pool = &state.db_pool;
    assert_eq!(
        scalar(
            pool,
            "SELECT COUNT(*) FROM compras WHERE usuario_email = $1"
        )
        .await,
        1
    );
    let metodo: String =
        sqlx::query_scalar("SELECT metodo_pago FROM compras WHERE usuario_email = $1")
            .bind(EMAIL)
            .fetch_one(pool)
            .await
            .unwrap();
    assert_eq!(metodo, "TARJETA BANCARIA");
    let (guardada, nombre): (Vec<u8>, String) = sqlx::query_as(
        "SELECT ticket_pdf, ticket_nombre_archivo FROM tickets_pdf WHERE numero_factura = '0000-000-000001'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!((guardada, nombre.as_str()), (photo, "IMG_0001.jpg"));
}

#[sqlx::test(migrations = "./migrations")]
async fn una_foto_grande_y_sin_extension_tambien_se_guarda(pool: PgPool) {
    let (_fake, state, app) = setup(pool).await;
    // ~6 MB de foto -> ~8 MB de JSON (el límite por defecto de axum era 2 MB)
    let photo = fake_photo(&vec![7u8; 6 * 1024 * 1024]);
    let mut ticket = reviewed_ticket(&photo);
    ticket["file_name"] = json!("foto pegada");

    let reply = send(&app, post_json("/api/ocr/confirm", EMAIL, &ticket)).await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json());
    let nombre: String = sqlx::query_scalar("SELECT ticket_nombre_archivo FROM tickets_pdf")
        .fetch_one(&state.db_pool)
        .await
        .unwrap();
    assert_eq!(nombre, "foto pegada.jpg");
}

#[sqlx::test(migrations = "./migrations")]
async fn la_revision_editable_es_solo_para_fotos(pool: PgPool) {
    let (_fake, state, app) = setup(pool).await;

    let reply = send(
        &app,
        post_json(
            "/api/ocr/confirm",
            EMAIL,
            &reviewed_ticket(&fake_pdf("0000-000-000001", "7.09")),
        ),
    )
    .await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert!(reply.json()["error"]
        .as_str()
        .unwrap()
        .contains("Solo las fotos"));
    assert_eq!(
        scalar(
            &state.db_pool,
            "SELECT COUNT(*) FROM compras WHERE usuario_email = $1"
        )
        .await,
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn el_usuario_demo_no_puede_guardar_fotos(pool: PgPool) {
    let (_fake, _state, app) = setup(pool).await;

    let reply = send(
        &app,
        post_json(
            "/api/ocr/confirm",
            DEMO,
            &reviewed_ticket(&fake_photo(b"x")),
        ),
    )
    .await;

    assert_eq!(reply.status, StatusCode::FORBIDDEN);
}

#[sqlx::test(migrations = "./migrations")]
async fn una_revision_incoherente_se_rechaza_con_un_motivo_claro(pool: PgPool) {
    let (_fake, state, app) = setup(pool).await;
    let photo = fake_photo(b"x");

    // El total no cuadra con la suma de productos
    let mut ticket = reviewed_ticket(&photo);
    ticket["total"] = json!(9.99);
    let reply = send(&app, post_json("/api/ocr/confirm", EMAIL, &ticket)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(reply.json()["error"]
        .as_str()
        .unwrap()
        .contains("no coincide"));

    // Un importe que no es cantidad × precio unitario
    let mut ticket = reviewed_ticket(&photo);
    ticket["productos"][0]["precio_total"] = json!(2.90);
    ticket["total"] = json!(8.09);
    let reply = send(&app, post_json("/api/ocr/confirm", EMAIL, &ticket)).await;
    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(reply.json()["error"]
        .as_str()
        .unwrap()
        .contains("«LECHE ENTERA»"));

    assert_eq!(
        scalar(
            &state.db_pool,
            "SELECT COUNT(*) FROM compras WHERE usuario_email = $1"
        )
        .await,
        0
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn una_foto_que_no_es_un_ticket_explica_el_motivo(pool: PgPool) {
    let (_fake, _state, app) = setup(pool).await;
    let body = json!({
        "ticket_id": "t1",
        "file_name": "gato.jpg",
        "file_content_b64": STANDARD.encode(fake_photo(b"NOTICKET")),
        "mime_type": "image/jpeg"
    });

    let reply = send(&app, post_json("/api/ocr/process", EMAIL, &body)).await;

    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        reply.json()["error"],
        "La imagen no parece un ticket de Mercadona."
    );
}

// ---------------------------------------------------------------------------
// PDF: el camino de siempre
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn los_pdf_se_procesan_y_guardan_como_siempre(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let body = json!({
        "ticket_id": "t1",
        "file_name": "ticket.pdf",
        "pdf_b64": STANDARD.encode(fake_pdf("0000-000-000002", "3.45")),
        "mime_type": "application/pdf",
        "usuario_email": EMAIL
    });

    let reply = send(&app, post_json("/api/ocr/process", EMAIL, &body)).await;

    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json());
    let body = reply.json();
    assert_eq!(body["ocr"]["numero_factura"], "0000-000-000002");
    assert_eq!(body["ocr"]["productos_detectados"], 1);
    assert_eq!(body["ingestion"]["ingested"], true);
    assert_eq!(fake.with(|s| s.ocr_calls), 1);
    assert_eq!(
        scalar(
            &state.db_pool,
            "SELECT COUNT(*) FROM compras WHERE usuario_email = $1"
        )
        .await,
        1
    );

    // Un PDF que el lector no entiende mantiene el mensaje genérico de siempre
    let broken = json!({
        "ticket_id": "t2",
        "file_name": "roto.pdf",
        "pdf_b64": STANDARD.encode(b"%PDF-1.4\nBROKEN"),
        "mime_type": "application/pdf"
    });
    let reply = send(&app, post_json("/api/ocr/process", EMAIL, &broken)).await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.json()["error"], "No se pudo procesar el ticket");
}
