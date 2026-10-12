//! Pruebas de integración del inicio de sesión con Google y de la sincronización de Gmail.
//!
//! Corren contra la base de datos real (`#[sqlx::test]`) y un Google/Gmail/OCR simulados
//! (`test_support`), de modo que recorren el camino completo -start -> callback -> BD ->
//! sincronización -> ticket ingerido- sin credenciales ni red. Datos 100 % sintéticos.

use std::collections::HashMap;
use std::time::Duration;

use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use reqwest::Url;
use serde_json::json;
use sqlx::PgPool;

use crate::{
    db,
    routes::auth::AppState,
    services::{
        crypto::pkce_challenge,
        gmail_sync::{sync_all, sync_user, SyncError, MAX_ATTEMPTS},
        hash_password, verify_jwt,
    },
    test_support::*,
};

// ---------------------------------------------------------------------------
// Ayudantes
// ---------------------------------------------------------------------------

struct Flow {
    state: String,
    challenge: String,
    cookie: String,
}

/// `GET /api/auth/google/start`: lo que haría el navegador al pulsar el botón.
async fn start_login(app: &axum::Router) -> Flow {
    let reply = send(app, get_req("/api/auth/google/start", None, None)).await;
    assert_eq!(
        reply.status,
        StatusCode::FOUND,
        "start debe redirigir a Google"
    );
    flow_from(&reply)
}

fn flow_from(reply: &Reply) -> Flow {
    let url = Url::parse(&reply.location()).expect("URL de Google");
    let q: HashMap<String, String> = url.query_pairs().into_owned().collect();
    Flow {
        state: q["state"].clone(),
        challenge: q["code_challenge"].clone(),
        cookie: reply.cookie_pair().expect("cookie de estado"),
    }
}

async fn callback(app: &axum::Router, code: &str, flow: &Flow) -> Reply {
    callback_with(app, code, &flow.state, Some(&flow.cookie)).await
}

async fn callback_with(app: &axum::Router, code: &str, state: &str, cookie: Option<&str>) -> Reply {
    send(
        app,
        get_req(
            &format!("/api/auth/google/callback?code={code}&state={state}"),
            cookie,
            None,
        ),
    )
    .await
}

/// Parámetros del fragmento (`/auth/google#a=1&b=2`).
fn fragment(location: &str) -> HashMap<String, String> {
    let frag = location.split_once('#').map(|(_, f)| f).unwrap_or_default();
    Url::parse(&format!("http://x/?{frag}"))
        .unwrap()
        .query_pairs()
        .into_owned()
        .collect()
}

async fn setup(pool: PgPool) -> (FakeGoogle, AppState, axum::Router) {
    let fake = FakeGoogle::start().await;
    let state = test_state(pool, &fake);
    let app = app(&state);
    (fake, state, app)
}

/// Usuario sin contraseña con Google conectado y un refresh token válido en el Google simulado.
async fn seed_connected_user(state: &AppState, fake: &FakeGoogle, email: &str, sub: &str) {
    let refresh = format!("1//rt-{sub}");
    fake.with(|s| {
        s.valid_refresh_tokens.insert(refresh.clone());
    });
    let cipher = state
        .google
        .as_ref()
        .unwrap()
        .cipher
        .encrypt(refresh.as_bytes(), email)
        .unwrap();
    db::google_accounts::create_user_with_link(
        &state.db_pool,
        email,
        Some("Ana Prueba"),
        sub,
        &cipher,
    )
    .await
    .unwrap();
}

async fn seed_password_user(state: &AppState, email: &str, password: &str) -> String {
    let hash = hash_password(password).unwrap();
    db::create_user(&state.db_pool, email, &hash, Some("Con Contraseña"))
        .await
        .unwrap();
    hash
}

async fn wait_idle(state: &AppState, email: &str) {
    let google = state.google.as_ref().unwrap();
    for _ in 0..400 {
        if !google.is_syncing(email) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("la sincronización no terminó a tiempo");
}

async fn count_purchases(pool: &PgPool, email: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM compras WHERE usuario_email = $1")
        .bind(email)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn count_users(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM usuarios")
        .fetch_one(pool)
        .await
        .unwrap()
}

fn json_post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

fn session_token(email: &str) -> String {
    crate::services::generate_jwt(email, JWT_SECRET).unwrap()
}

// ---------------------------------------------------------------------------
// Inicio de sesión con Google
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn login_con_google_crea_la_cuenta_cifra_el_token_y_emite_el_jwt(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    fake.add_code("code-1", FakeProfile::new("sub-ana", "Ana@Example.com"));

    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;

    assert_eq!(reply.status, StatusCode::FOUND);
    let loc = reply.location();
    assert!(
        loc.starts_with("/auth/google#"),
        "destino inesperado: {loc}"
    );
    let frag = fragment(&loc);
    assert!(!frag.contains_key("error"), "no debería haber error: {loc}");
    assert_eq!(
        frag["email"], "ana@example.com",
        "el email se normaliza a minúsculas"
    );
    assert_eq!(frag["is_demo"], "false");
    assert_eq!(
        verify_jwt(&frag["token"], JWT_SECRET).unwrap().sub,
        "ana@example.com"
    );

    // La cookie de estado se borra y la respuesta no se cachea
    assert!(reply.set_cookie().contains("Max-Age=0"));
    assert_eq!(reply.headers[header::CACHE_CONTROL], "no-store");

    // Cuenta sin contraseña, con el nombre de Google
    let (hash, nombre): (Option<String>, Option<String>) = sqlx::query_as(
        "SELECT password_hash, nombre FROM usuarios WHERE email = 'ana@example.com'",
    )
    .fetch_one(&state.db_pool)
    .await
    .unwrap();
    assert_eq!(hash, None);
    assert_eq!(nombre.as_deref(), Some("Ana Prueba"));

    // El refresh token está cifrado en la BD y descifra al valor original
    let account = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .expect("cuenta vinculada");
    let plain = b"1//rt-sub-ana";
    assert!(!account
        .refresh_token_cifrado
        .windows(plain.len())
        .any(|w| w == plain));
    let google = state.google.as_ref().unwrap();
    assert_eq!(
        google
            .cipher
            .decrypt(&account.refresh_token_cifrado, "ana@example.com")
            .unwrap(),
        plain
    );

    // PKCE: el verificador que recibió Google corresponde al desafío de la URL de autorización
    let verifier = fake
        .with(|s| s.last_code_verifier.clone())
        .expect("verificador");
    assert_eq!(pkce_challenge(&verifier), flow.challenge);

    wait_idle(&state, "ana@example.com").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn rechaza_un_callback_con_estado_incorrecto_o_sin_cookie(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    fake.add_code("code-1", FakeProfile::new("sub-ana", "ana@example.com"));
    let flow = start_login(&app).await;

    // state distinto al de la cookie
    let reply = callback_with(&app, "code-1", "estado-falso", Some(&flow.cookie)).await;
    assert_eq!(fragment(&reply.location())["error"], "estado_invalido");

    // sin cookie (otro navegador / flujo iniciado por un atacante)
    let reply = callback_with(&app, "code-1", &flow.state, None).await;
    assert_eq!(fragment(&reply.location())["error"], "estado_invalido");

    // cookie manipulada
    let reply = callback_with(
        &app,
        "code-1",
        &flow.state,
        Some(&format!("{}x", flow.cookie)),
    )
    .await;
    assert_eq!(fragment(&reply.location())["error"], "estado_invalido");

    assert_eq!(
        count_users(&state.db_pool).await,
        0,
        "no se crea ninguna cuenta"
    );
    assert_eq!(
        fake.with(|s| s.last_code_verifier.clone()),
        None,
        "ni se llegó a canjear el código"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn si_google_deniega_el_acceso_no_se_crea_nada(pool: PgPool) {
    let (_fake, state, app) = setup(pool).await;
    let flow = start_login(&app).await;
    let reply = send(
        &app,
        get_req(
            &format!(
                "/api/auth/google/callback?error=access_denied&state={}",
                flow.state
            ),
            Some(&flow.cookie),
            None,
        ),
    )
    .await;
    assert_eq!(fragment(&reply.location())["error"], "acceso_denegado");
    assert_eq!(count_users(&state.db_pool).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn exige_que_el_usuario_conceda_el_permiso_de_gmail(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let mut profile = FakeProfile::new("sub-ana", "ana@example.com");
    profile.grants_gmail = false; // desmarcó la casilla de Gmail
    fake.add_code("code-1", profile);

    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;

    assert_eq!(
        fragment(&reply.location())["error"],
        "permiso_gmail_denegado"
    );
    assert_eq!(count_users(&state.db_pool).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn rechaza_un_email_de_google_sin_verificar(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let mut profile = FakeProfile::new("sub-ana", "ana@example.com");
    profile.email_verified = false;
    fake.add_code("code-1", profile);

    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;

    assert_eq!(fragment(&reply.location())["error"], "email_no_verificado");
    assert_eq!(count_users(&state.db_pool).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn un_codigo_invalido_se_traduce_a_un_error_sin_filtrar_texto_de_google(pool: PgPool) {
    let (_fake, _state, app) = setup(pool).await;
    let flow = start_login(&app).await;
    let reply = callback(&app, "codigo-que-no-existe", &flow).await;
    assert_eq!(fragment(&reply.location())["error"], "codigo_invalido");
}

#[sqlx::test(migrations = "./migrations")]
async fn si_google_no_entrega_refresh_token_se_repite_el_consentimiento_una_vez(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let mut sin_token = FakeProfile::new("sub-ana", "ana@example.com");
    sin_token.refresh_token = None;
    fake.add_code("code-1", sin_token);
    fake.add_code("code-2", FakeProfile::new("sub-ana", "ana@example.com"));

    // Primer intento: sin refresh token -> vuelve a Google pidiendo consentimiento explícito
    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;
    assert_eq!(reply.status, StatusCode::FOUND);
    assert!(
        reply.location().contains("prompt=consent"),
        "{}",
        reply.location()
    );
    assert_eq!(
        count_users(&state.db_pool).await,
        0,
        "todavía no hay cuenta"
    );

    // Segundo intento (con el estado y la cookie nuevos): ya trae refresh token
    let retry = flow_from(&reply);
    assert_ne!(retry.state, flow.state, "el estado se renueva");
    let reply = callback(&app, "code-2", &retry).await;
    let frag = fragment(&reply.location());
    assert!(frag.contains_key("token"), "{}", reply.location());
    assert_eq!(count_users(&state.db_pool).await, 1);
    wait_idle(&state, "ana@example.com").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn si_tras_forzar_el_consentimiento_sigue_sin_refresh_token_falla_sin_bucle(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let mut sin_token = FakeProfile::new("sub-ana", "ana@example.com");
    sin_token.refresh_token = None;
    fake.add_code("code-1", sin_token.clone());
    fake.add_code("code-2", sin_token);

    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;
    let retry = flow_from(&reply);
    let reply = callback(&app, "code-2", &retry).await;

    assert_eq!(fragment(&reply.location())["error"], "sin_refresh_token");
    assert_eq!(count_users(&state.db_pool).await, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn google_no_enlaza_sola_una_cuenta_con_contrasena_ya_existente(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    // Alguien registró ana@example.com con contraseña (el registro no verifica el email)
    let hash = seed_password_user(&state, "ana@example.com", "clave-del-atacante-1").await;
    fake.add_code("code-1", FakeProfile::new("sub-ana", "ana@example.com"));

    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;

    let frag = fragment(&reply.location());
    assert_eq!(frag["error"], "cuenta_existente");
    assert!(!frag.contains_key("token"), "no se entrega ninguna sesión");
    assert!(
        db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
            .await
            .unwrap()
            .is_none()
    );
    let stored: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM usuarios WHERE email = 'ana@example.com'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
    assert_eq!(stored, Some(hash), "la cuenta existente queda intacta");
}

#[sqlx::test(migrations = "./migrations")]
async fn una_cuenta_de_google_ya_vinculada_inicia_sesion_y_renueva_el_token(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    let before = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .unwrap();

    // Google entrega ahora un refresh token distinto
    let mut profile = FakeProfile::new("sub-ana", "ana@example.com");
    profile.refresh_token = Some("1//rt-renovado".into());
    fake.add_code("code-1", profile);

    let flow = start_login(&app).await;
    let reply = callback(&app, "code-1", &flow).await;
    assert!(fragment(&reply.location()).contains_key("token"));
    assert_eq!(
        count_users(&state.db_pool).await,
        1,
        "no se duplica el usuario"
    );

    let after = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_ne!(before.refresh_token_cifrado, after.refresh_token_cifrado);
    let google = state.google.as_ref().unwrap();
    assert_eq!(
        google
            .cipher
            .decrypt(&after.refresh_token_cifrado, "ana@example.com")
            .unwrap(),
        b"1//rt-renovado"
    );
    wait_idle(&state, "ana@example.com").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn una_cuenta_creada_con_google_no_puede_entrar_con_contrasena(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;

    let reply = send(
        &app,
        json_post(
            "/api/auth/login",
            json!({"email": "ana@example.com", "password": "lo-que-sea-123"}),
        ),
    )
    .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert!(reply.json()["error"].as_str().unwrap().contains("Google"));
}

// ---------------------------------------------------------------------------
// Conectar Gmail desde una sesión ya iniciada
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn un_usuario_con_contrasena_puede_conectar_google_desde_su_sesion(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    let hash = seed_password_user(&state, "ana@example.com", "clave-segura-123").await;
    // Su Gmail no tiene por qué coincidir con el email de la app
    fake.add_code(
        "code-1",
        FakeProfile::new("sub-ana", "ana.personal@gmail.test"),
    );

    let token = session_token("ana@example.com");
    let reply = send(&app, authed("POST", "/api/gmail/connect", &token)).await;
    assert_eq!(reply.status, StatusCode::OK);
    let url = reply.json()["url"].as_str().unwrap().to_string();
    assert!(url.contains("access_type=offline"));
    let flow = Flow {
        state: Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned(),
        challenge: String::new(),
        cookie: reply.cookie_pair().unwrap(),
    };

    let reply = callback(&app, "code-1", &flow).await;
    let frag = fragment(&reply.location());
    assert_eq!(
        frag.get("linked").map(String::as_str),
        Some("1"),
        "{}",
        reply.location()
    );
    assert!(
        !frag.contains_key("token"),
        "vincular no emite una sesión nueva"
    );

    let account = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.google_email, "ana.personal@gmail.test");
    let stored: Option<String> =
        sqlx::query_scalar("SELECT password_hash FROM usuarios WHERE email = 'ana@example.com'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
    assert_eq!(stored, Some(hash), "la contraseña no se toca");
    wait_idle(&state, "ana@example.com").await;

    // El estado refleja la conexión
    let status = send(&app, authed("GET", "/api/gmail/status", &token))
        .await
        .json();
    assert_eq!(status["disponible"], true);
    assert_eq!(status["conectado"], true);
    assert_eq!(status["google_email"], "ana.personal@gmail.test");
    assert_eq!(status["estado"], "activa");
    assert_eq!(status["remitente"], "ticket_digital@mail.mercadona.com");
}

#[sqlx::test(migrations = "./migrations")]
async fn una_cuenta_de_google_no_se_puede_vincular_a_dos_usuarios(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-compartida").await;
    seed_password_user(&state, "luis@example.com", "clave-segura-123").await;
    fake.add_code(
        "code-1",
        FakeProfile::new("sub-compartida", "ana@example.com"),
    );

    let token = session_token("luis@example.com");
    let reply = send(&app, authed("POST", "/api/gmail/connect", &token)).await;
    let url = reply.json()["url"].as_str().unwrap().to_string();
    let flow = Flow {
        state: Url::parse(&url)
            .unwrap()
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .into_owned(),
        challenge: String::new(),
        cookie: reply.cookie_pair().unwrap(),
    };
    let reply = callback(&app, "code-1", &flow).await;

    assert_eq!(fragment(&reply.location())["error"], "google_en_uso");
    assert!(
        db::google_accounts::find_by_user(&state.db_pool, "luis@example.com")
            .await
            .unwrap()
            .is_none()
    );
    wait_idle(&state, "ana@example.com").await;
}

#[sqlx::test(migrations = "./migrations")]
async fn el_usuario_demo_no_puede_conectar_gmail(pool: PgPool) {
    let (_fake, state, app) = setup(pool).await;
    seed_password_user(&state, "demo@example.com", "clave-segura-123").await;
    let token = session_token("demo@example.com");

    for (method, uri) in [
        ("POST", "/api/gmail/connect"),
        ("POST", "/api/gmail/sync"),
        ("DELETE", "/api/gmail/connection"),
    ] {
        let reply = send(&app, authed(method, uri, &token)).await;
        assert_eq!(reply.status, StatusCode::FORBIDDEN, "{method} {uri}");
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn los_endpoints_de_gmail_exigen_sesion(pool: PgPool) {
    let (_fake, _state, app) = setup(pool).await;
    for (method, uri) in [
        ("POST", "/api/gmail/connect"),
        ("GET", "/api/gmail/status"),
        ("POST", "/api/gmail/sync"),
        ("DELETE", "/api/gmail/connection"),
    ] {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app, request).await.status,
            StatusCode::UNAUTHORIZED,
            "{method} {uri}"
        );
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn sin_credenciales_de_google_la_integracion_queda_desactivada(pool: PgPool) {
    let fake = FakeGoogle::start().await;
    let mut state = test_state(pool, &fake);
    state.google = None;
    let app = app(&state);

    let providers = send(&app, get_req("/api/auth/providers", None, None))
        .await
        .json();
    assert_eq!(providers["google"], false);

    let reply = send(&app, get_req("/api/auth/google/start", None, None)).await;
    assert_eq!(fragment(&reply.location())["error"], "no_configurado");

    seed_password_user(&state, "ana@example.com", "clave-segura-123").await;
    let token = session_token("ana@example.com");
    let status = send(&app, authed("GET", "/api/gmail/status", &token))
        .await
        .json();
    assert_eq!(status["disponible"], false);
    assert_eq!(status["conectado"], false);
    assert_eq!(
        send(&app, authed("POST", "/api/gmail/connect", &token))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn con_credenciales_el_boton_de_google_esta_disponible(pool: PgPool) {
    let (_fake, _state, app) = setup(pool).await;
    let providers = send(&app, get_req("/api/auth/providers", None, None))
        .await
        .json();
    assert_eq!(providers["google"], true);
}

// ---------------------------------------------------------------------------
// Sincronización
// ---------------------------------------------------------------------------

#[sqlx::test(migrations = "./migrations")]
async fn la_sincronizacion_importa_los_tickets_y_es_idempotente(pool: PgPool) {
    let (fake, state, _app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    let email = "ana@example.com";

    fake.with(|s| {
        s.mailbox = vec![
            // 1. ticket legítimo
            FakeMessage::ticket("m1", "4567-001-000001", "12.50"),
            // 2. nombre visible = remitente de Mercadona, pero la dirección real es otra
            FakeMessage {
                id: "m2".into(),
                from: "ticket_digital@mail.mercadona.com <atacante@evil.example>".into(),
                attachments: vec![FakeAttachment {
                    filename: "ticket.pdf".into(),
                    mime: "application/pdf".into(),
                    data: fake_pdf("9999-001-000009", "99.00"),
                }],
            },
            // 3. remitente legítimo pero sin PDF
            FakeMessage {
                id: "m3".into(),
                from: "ticket_digital@mail.mercadona.com".into(),
                attachments: vec![FakeAttachment {
                    filename: "logo.png".into(),
                    mime: "image/png".into(),
                    data: vec![1, 2, 3],
                }],
            },
            // 4. PDF que el OCR no sabe leer
            FakeMessage::ticket("m4", "BROKEN", "1.00"),
            // 5. "PDF" que en realidad no lo es: ni siquiera llega al OCR
            FakeMessage {
                id: "m5".into(),
                from: "ticket_digital@mail.mercadona.com".into(),
                attachments: vec![FakeAttachment {
                    filename: "ticket.pdf".into(),
                    mime: "application/pdf".into(),
                    data: b"esto no es un pdf".to_vec(),
                }],
            },
        ];
    });

    let report = sync_user(&state, email)
        .await
        .expect("sincronización correcta");
    assert_eq!(report.counts.importados, 1);
    assert_eq!(report.counts.fallidos, 2, "m4 y m5");
    assert_eq!(report.counts.duplicados, 0);
    assert_eq!(report.sin_pdf, 2, "m2 (remitente falso) y m3");
    assert_eq!(report.pendientes, 0);
    assert_eq!(
        fake.with(|s| s.last_query.clone()).as_deref(),
        Some("from:ticket_digital@mail.mercadona.com has:attachment filename:pdf")
    );

    // El ticket legítimo quedó en la BD de ese usuario, con su PDF
    assert_eq!(count_purchases(&state.db_pool, email).await, 1);
    let total: rust_decimal::Decimal =
        sqlx::query_scalar("SELECT total FROM compras WHERE numero_factura = '4567-001-000001'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
    assert_eq!(total.to_string(), "12.50");
    let stored_pdf: Vec<u8> = sqlx::query_scalar(
        "SELECT ticket_pdf FROM tickets_pdf WHERE numero_factura = '4567-001-000001'",
    )
    .fetch_one(&state.db_pool)
    .await
    .unwrap();
    assert_eq!(stored_pdf, fake_pdf("4567-001-000001", "12.50"));
    // y el del remitente falso NO
    let spoofed: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM compras WHERE numero_factura = '9999-001-000009'")
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
    assert_eq!(spoofed, 0);

    // Estado persistido para el panel
    let account = db::google_accounts::find_by_user(&state.db_pool, email)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.ultima_sync_ok, Some(true));
    assert_eq!(account.ultima_sync_importados, 1);
    assert_eq!(account.ultima_sync_fallidos, 2);
    assert!(account.ultima_sync_fin.is_some());

    // Segunda ejecución: lo ya visto no se vuelve a descargar; solo se reintentan los fallidos
    let gets_before = fake.with(|s| s.gmail_get_calls);
    let second = sync_user(&state, email).await.unwrap();
    assert_eq!(second.counts.importados, 0);
    assert_eq!(second.counts.fallidos, 2);
    assert_eq!(
        fake.with(|s| s.gmail_get_calls) - gets_before,
        2,
        "solo m4 y m5"
    );
    assert_eq!(
        count_purchases(&state.db_pool, email).await,
        1,
        "sin duplicar"
    );

    // Tras MAX_ATTEMPTS intentos, los fallidos se dejan de reintentar
    for _ in 0..(MAX_ATTEMPTS as usize) {
        let _ = sync_user(&state, email).await.unwrap();
    }
    let gets_before = fake.with(|s| s.gmail_get_calls);
    let last = sync_user(&state, email).await.unwrap();
    assert_eq!(last.counts, Default::default());
    assert_eq!(
        fake.with(|s| s.gmail_get_calls),
        gets_before,
        "ya no se toca ningún mensaje"
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn un_ticket_ya_subido_a_mano_cuenta_como_duplicado(pool: PgPool) {
    let (fake, state, _app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| s.mailbox = vec![FakeMessage::ticket("m1", "4567-001-000001", "12.50")]);

    assert_eq!(
        sync_user(&state, "ana@example.com")
            .await
            .unwrap()
            .counts
            .importados,
        1
    );

    // Olvida los correos vistos: el ticket sigue en la BD (como si lo hubiera subido a mano)
    sqlx::query("DELETE FROM gmail_mensajes_procesados")
        .execute(&state.db_pool)
        .await
        .unwrap();
    let report = sync_user(&state, "ana@example.com").await.unwrap();
    assert_eq!(report.counts.importados, 0);
    assert_eq!(report.counts.duplicados, 1);
    assert_eq!(count_purchases(&state.db_pool, "ana@example.com").await, 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn si_el_ocr_esta_caido_se_corta_y_los_correos_se_reintentan_despues(pool: PgPool) {
    let (fake, state, _app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| {
        s.mailbox = vec![FakeMessage::ticket("m1", "4567-001-000001", "12.50")];
        s.ocr_down = true;
    });

    let err = sync_user(&state, "ana@example.com").await.unwrap_err();
    assert_eq!(err, SyncError::Failed("ocr_no_disponible"));
    let processed = db::google_accounts::processed_messages(&state.db_pool, "ana@example.com")
        .await
        .unwrap();
    assert!(
        processed.is_empty(),
        "un fallo transitorio no marca el correo como visto"
    );
    let account = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.ultima_sync_ok, Some(false));
    assert_eq!(
        account.ultima_sync_error.as_deref(),
        Some("ocr_no_disponible")
    );
    assert_eq!(account.estado, "activa", "no hace falta reconectar");

    fake.with(|s| s.ocr_down = false);
    assert_eq!(
        sync_user(&state, "ana@example.com")
            .await
            .unwrap()
            .counts
            .importados,
        1
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn un_token_revocado_marca_la_cuenta_para_reconectar_y_deja_de_sincronizarla(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| s.valid_refresh_tokens.clear()); // el usuario revocó el acceso en Google

    let err = sync_user(&state, "ana@example.com").await.unwrap_err();
    assert_eq!(err, SyncError::NeedsReconnect);
    let account = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.estado, "requiere_reconexion");
    assert_eq!(account.ultima_sync_error.as_deref(), Some("token_revocado"));

    // No se vuelve a molestar a Google con un token muerto, ni el planificador la incluye
    let calls = fake.with(|s| s.refresh_calls);
    assert_eq!(
        sync_user(&state, "ana@example.com").await.unwrap_err(),
        SyncError::NeedsReconnect
    );
    assert_eq!(fake.with(|s| s.refresh_calls), calls);
    assert!(db::google_accounts::list_active_emails(&state.db_pool)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(sync_all(&state).await, (0, 0));

    // El panel lo refleja y "sincronizar ahora" pide reconectar
    let token = session_token("ana@example.com");
    let status = send(&app, authed("GET", "/api/gmail/status", &token))
        .await
        .json();
    assert_eq!(status["estado"], "requiere_reconexion");
    assert_eq!(status["ultima_sync_error"], "token_revocado");
    assert_eq!(
        send(&app, authed("POST", "/api/gmail/sync", &token))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn si_cambia_la_clave_de_cifrado_la_cuenta_pide_reconectar(pool: PgPool) {
    let (fake, state, _app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    // Simula un token cifrado con otra clave
    let otro = crate::services::crypto::TokenCipher::new(&[1; 32])
        .encrypt(b"1//rt-sub-ana", "ana@example.com")
        .unwrap();
    db::google_accounts::update_refresh_token(&state.db_pool, "ana@example.com", &otro)
        .await
        .unwrap();

    assert_eq!(
        sync_user(&state, "ana@example.com").await.unwrap_err(),
        SyncError::NeedsReconnect
    );
    let account = db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(account.estado, "requiere_reconexion");
}

#[sqlx::test(migrations = "./migrations")]
async fn si_gmail_deniega_el_acceso_se_informa_con_un_codigo(pool: PgPool) {
    let (fake, state, _app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| s.gmail_forbidden = true); // p. ej. la API de Gmail no está habilitada en el proyecto

    assert_eq!(
        sync_user(&state, "ana@example.com").await.unwrap_err(),
        SyncError::Failed("gmail_denegado")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sincronizar_ahora_corre_en_segundo_plano_y_no_se_solapa(pool: PgPool) {
    let (fake, state, app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| s.mailbox = vec![FakeMessage::ticket("m1", "4567-001-000001", "12.50")]);
    let token = session_token("ana@example.com");

    // Con una sincronización en curso, otra petición no arranca una segunda
    let permit = state
        .google
        .as_ref()
        .unwrap()
        .try_begin_sync("ana@example.com")
        .unwrap();
    let reply = send(&app, authed("POST", "/api/gmail/sync", &token)).await;
    assert_eq!(reply.status, StatusCode::ACCEPTED);
    assert_eq!(reply.json()["iniciada"], false);
    assert_eq!(reply.json()["motivo"], "ya_en_curso");
    let status = send(&app, authed("GET", "/api/gmail/status", &token))
        .await
        .json();
    assert_eq!(status["sincronizando"], true);
    drop(permit);

    // Libre: arranca, responde enseguida y termina por su cuenta
    let reply = send(&app, authed("POST", "/api/gmail/sync", &token)).await;
    assert_eq!(reply.status, StatusCode::ACCEPTED);
    assert_eq!(reply.json()["iniciada"], true);
    wait_idle(&state, "ana@example.com").await;
    assert_eq!(count_purchases(&state.db_pool, "ana@example.com").await, 1);

    let status = send(&app, authed("GET", "/api/gmail/status", &token))
        .await
        .json();
    assert_eq!(status["sincronizando"], false);
    assert_eq!(status["ultima_sync_ok"], true);
    assert_eq!(status["ultima_sync_importados"], 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn desconectar_revoca_el_acceso_en_google_borra_el_token_y_conserva_los_tickets(
    pool: PgPool,
) {
    let (fake, state, app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| s.mailbox = vec![FakeMessage::ticket("m1", "4567-001-000001", "12.50")]);
    sync_user(&state, "ana@example.com").await.unwrap();
    let token = session_token("ana@example.com");

    let reply = send(&app, authed("DELETE", "/api/gmail/connection", &token)).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["revocado_en_google"], true);
    assert_eq!(
        fake.with(|s| s.revoked.clone()),
        vec!["1//rt-sub-ana".to_string()]
    );

    assert!(
        db::google_accounts::find_by_user(&state.db_pool, "ana@example.com")
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        count_purchases(&state.db_pool, "ana@example.com").await,
        1,
        "los tickets importados se quedan"
    );
    let status = send(&app, authed("GET", "/api/gmail/status", &token))
        .await
        .json();
    assert_eq!(status["conectado"], false);

    // Idempotente
    assert_eq!(
        send(&app, authed("DELETE", "/api/gmail/connection", &token))
            .await
            .status,
        StatusCode::OK
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn sin_conectar_no_se_puede_sincronizar_ahora(pool: PgPool) {
    let (_fake, state, app) = setup(pool).await;
    seed_password_user(&state, "ana@example.com", "clave-segura-123").await;
    let token = session_token("ana@example.com");
    assert_eq!(
        send(&app, authed("POST", "/api/gmail/sync", &token))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        sync_user(&state, "ana@example.com").await.unwrap_err(),
        SyncError::NotConnected
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn borrar_al_usuario_borra_su_token_y_su_historial_de_correos(pool: PgPool) {
    let (fake, state, _app) = setup(pool).await;
    seed_connected_user(&state, &fake, "ana@example.com", "sub-ana").await;
    fake.with(|s| s.mailbox = vec![FakeMessage::ticket("m1", "4567-001-000001", "12.50")]);
    sync_user(&state, "ana@example.com").await.unwrap();

    sqlx::query("DELETE FROM usuarios WHERE email = 'ana@example.com'")
        .execute(&state.db_pool)
        .await
        .unwrap();
    for table in ["cuentas_google", "gmail_mensajes_procesados", "compras"] {
        let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
            .fetch_one(&state.db_pool)
            .await
            .unwrap();
        assert_eq!(n, 0, "{table} debe vaciarse en cascada");
    }
}
