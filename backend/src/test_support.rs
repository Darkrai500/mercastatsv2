//! Utilidades de test: un servidor local que imita a Google (tokens, userinfo, revoke), a la
//! API de Gmail y al servicio OCR, más helpers para llamar a los routers sin abrir puertos.
//! Todos los datos son sintéticos.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use axum::{
    body::{to_bytes, Body},
    extract::{Path, Query, State},
    http::{header, HeaderMap, Request, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Form, Json, Router,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine as _,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;

use crate::{
    config::{AppConfig, GoogleConfig, DEFAULT_TICKET_SENDER},
    routes::{self, auth::AppState},
    services::{google_integration::GoogleIntegration, IntelligenceClient},
};

pub const JWT_SECRET: &str = "secreto-de-pruebas-0123456789-abcdef";
pub const GMAIL_SCOPE: &str = "https://www.googleapis.com/auth/gmail.readonly";

// ---------------------------------------------------------------------------
// Datos del Google simulado
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct FakeProfile {
    pub sub: String,
    pub email: String,
    pub email_verified: bool,
    pub name: Option<String>,
    pub refresh_token: Option<String>,
    pub grants_gmail: bool,
}

impl FakeProfile {
    pub fn new(sub: &str, email: &str) -> Self {
        Self {
            sub: sub.into(),
            email: email.into(),
            email_verified: true,
            name: Some("Ana Prueba".into()),
            refresh_token: Some(format!("1//rt-{sub}")),
            grants_gmail: true,
        }
    }
}

#[derive(Clone)]
pub struct FakeAttachment {
    pub filename: String,
    pub mime: String,
    pub data: Vec<u8>,
}

#[derive(Clone)]
pub struct FakeMessage {
    pub id: String,
    pub from: String,
    pub attachments: Vec<FakeAttachment>,
}

impl FakeMessage {
    /// Correo legítimo de Mercadona con un ticket PDF sintético.
    pub fn ticket(id: &str, factura: &str, total: &str) -> Self {
        Self {
            id: id.into(),
            from: format!("Mercadona <{DEFAULT_TICKET_SENDER}>"),
            attachments: vec![FakeAttachment {
                filename: "ticket.pdf".into(),
                mime: "application/pdf".into(),
                data: fake_pdf(factura, total),
            }],
        }
    }
}

/// PDF falso: el OCR simulado lee la factura y el total de dentro.
pub fn fake_pdf(factura: &str, total: &str) -> Vec<u8> {
    format!("%PDF-1.4\nFAKE:{factura}:{total}\n").into_bytes()
}

#[derive(Default)]
pub struct FakeState {
    /// código de autorización -> perfil que Google devolvería
    pub codes: HashMap<String, FakeProfile>,
    pub valid_refresh_tokens: HashSet<String>,
    pub mailbox: Vec<FakeMessage>,
    pub revoked: Vec<String>,
    pub ocr_down: bool,
    pub gmail_forbidden: bool,
    // Observaciones
    pub ocr_calls: usize,
    pub refresh_calls: usize,
    pub gmail_get_calls: usize,
    pub last_query: Option<String>,
    pub last_code_verifier: Option<String>,
}

pub type Shared = Arc<Mutex<FakeState>>;

pub struct FakeGoogle {
    pub base_url: String,
    pub state: Shared,
}

impl FakeGoogle {
    pub async fn start() -> Self {
        let state: Shared = Arc::new(Mutex::new(FakeState::default()));
        let app = Router::new()
            .route("/token", post(token))
            .route("/userinfo", get(userinfo))
            .route("/revoke", post(revoke))
            .route("/gmail/v1/users/me/messages", get(gmail_list))
            .route("/gmail/v1/users/me/messages/:id", get(gmail_get))
            .route(
                "/gmail/v1/users/me/messages/:id/attachments/:att",
                get(gmail_attachment),
            )
            .route("/ocr/process", post(ocr_process))
            .with_state(state.clone());

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        Self { base_url, state }
    }

    pub fn with<R>(&self, f: impl FnOnce(&mut FakeState) -> R) -> R {
        f(&mut self.state.lock().unwrap())
    }

    /// Registra un código de autorización que Google canjearía por este perfil.
    pub fn add_code(&self, code: &str, profile: FakeProfile) {
        self.with(|s| {
            if let Some(rt) = &profile.refresh_token {
                s.valid_refresh_tokens.insert(rt.clone());
            }
            s.codes.insert(code.into(), profile);
        });
    }
}

fn scope_of(profile: &FakeProfile) -> String {
    if profile.grants_gmail {
        format!("openid https://www.googleapis.com/auth/userinfo.email {GMAIL_SCOPE}")
    } else {
        "openid https://www.googleapis.com/auth/userinfo.email".to_string()
    }
}

async fn token(
    State(s): State<Shared>,
    Form(form): Form<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut state = s.lock().unwrap();
    let invalid = || {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant"})),
        )
            .into_response()
    };

    match form.get("grant_type").map(String::as_str) {
        Some("authorization_code") => {
            state.last_code_verifier = form.get("code_verifier").cloned();
            let code = form.get("code").cloned().unwrap_or_default();
            let Some(profile) = state.codes.get(&code) else {
                return invalid();
            };
            let mut body = json!({
                "access_token": format!("at-{code}"),
                "expires_in": 3599,
                "token_type": "Bearer",
                "scope": scope_of(profile),
            });
            if let Some(rt) = &profile.refresh_token {
                body["refresh_token"] = json!(rt);
            }
            Json(body).into_response()
        }
        Some("refresh_token") => {
            state.refresh_calls += 1;
            let rt = form.get("refresh_token").cloned().unwrap_or_default();
            if state.valid_refresh_tokens.contains(&rt) {
                Json(
                    json!({"access_token": "at-refresh", "expires_in": 3599, "scope": GMAIL_SCOPE}),
                )
                .into_response()
            } else {
                invalid()
            }
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "unsupported_grant_type"})),
        )
            .into_response(),
    }
}

async fn userinfo(State(s): State<Shared>, headers: HeaderMap) -> impl IntoResponse {
    let state = s.lock().unwrap();
    let bearer = bearer(&headers);
    let profile = bearer
        .strip_prefix("at-")
        .and_then(|code| state.codes.get(code));
    match profile {
        Some(p) => Json(json!({
            "sub": p.sub, "email": p.email, "email_verified": p.email_verified, "name": p.name,
        }))
        .into_response(),
        None => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn revoke(State(s): State<Shared>, Form(form): Form<HashMap<String, String>>) -> StatusCode {
    let mut state = s.lock().unwrap();
    let token = form.get("token").cloned().unwrap_or_default();
    state.valid_refresh_tokens.remove(&token);
    state.revoked.push(token);
    StatusCode::OK
}

fn bearer(headers: &HeaderMap) -> String {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_string()
}

fn gmail_guard(state: &FakeState, headers: &HeaderMap) -> Option<StatusCode> {
    if !bearer(headers).starts_with("at-") {
        return Some(StatusCode::UNAUTHORIZED);
    }
    if state.gmail_forbidden {
        return Some(StatusCode::FORBIDDEN);
    }
    None
}

async fn gmail_list(
    State(s): State<Shared>,
    headers: HeaderMap,
    Query(q): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let mut state = s.lock().unwrap();
    if let Some(status) = gmail_guard(&state, &headers) {
        return status.into_response();
    }
    state.last_query = q.get("q").cloned();
    let messages: Vec<Value> = state
        .mailbox
        .iter()
        .map(|m| json!({"id": m.id, "threadId": m.id}))
        .collect();
    if messages.is_empty() {
        return Json(json!({"resultSizeEstimate": 0})).into_response();
    }
    Json(json!({"messages": messages, "resultSizeEstimate": messages.len()})).into_response()
}

async fn gmail_get(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let mut state = s.lock().unwrap();
    if let Some(status) = gmail_guard(&state, &headers) {
        return status.into_response();
    }
    state.gmail_get_calls += 1;
    let Some(message) = state.mailbox.iter().find(|m| m.id == id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let mut parts = vec![json!({
        "mimeType": "text/html", "filename": "",
        "body": {"size": 12, "data": URL_SAFE_NO_PAD.encode("<p>Gracias</p>")}
    })];
    for (index, att) in message.attachments.iter().enumerate() {
        parts.push(json!({
            "mimeType": att.mime, "filename": att.filename,
            "body": {"attachmentId": format!("att-{index}"), "size": att.data.len()}
        }));
    }
    Json(json!({
        "id": message.id,
        "payload": {
            "mimeType": "multipart/mixed",
            "headers": [{"name": "From", "value": message.from}, {"name": "Subject", "value": "Tu ticket"}],
            "parts": parts,
        }
    }))
    .into_response()
}

async fn gmail_attachment(
    State(s): State<Shared>,
    headers: HeaderMap,
    Path((id, att)): Path<(String, String)>,
) -> impl IntoResponse {
    let state = s.lock().unwrap();
    if let Some(status) = gmail_guard(&state, &headers) {
        return status.into_response();
    }
    let index: usize = att.trim_start_matches("att-").parse().unwrap_or(usize::MAX);
    let data = state
        .mailbox
        .iter()
        .find(|m| m.id == id)
        .and_then(|m| m.attachments.get(index));
    match data {
        Some(a) => Json(json!({"size": a.data.len(), "data": URL_SAFE_NO_PAD.encode(&a.data)}))
            .into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn ocr_process(State(s): State<Shared>, Json(req): Json<Value>) -> impl IntoResponse {
    let mut state = s.lock().unwrap();
    state.ocr_calls += 1;
    if state.ocr_down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    let bytes = STANDARD
        .decode(req["file_content_b64"].as_str().unwrap_or_default())
        .unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes).to_string();
    if text.contains("NOTICKET") {
        // Así rechaza el lector de fotos una imagen que no es un ticket
        return (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(json!({
                "detail": "La imagen no parece un ticket de Mercadona.",
                "code": "ticket_rejected"
            })),
        )
            .into_response();
    }
    if text.contains("BROKEN") {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    }
    let Some(rest) = text.lines().find_map(|l| l.strip_prefix("FAKE:")) else {
        return StatusCode::UNPROCESSABLE_ENTITY.into_response();
    };
    let (factura, total) = rest.split_once(':').unwrap();
    let total: f64 = total.parse().unwrap();
    Json(json!({
        "ticket_id": req["ticket_id"],
        "raw_text": "",
        "numero_factura": factura,
        "fecha": "2026-03-02",
        "fecha_hora": "2026-03-02 18:30:00",
        "total": total,
        "tienda": "MERCADONA, S.A.",
        "ubicacion": "CALLE FICTICIA 1",
        "metodo_pago": "TARJETA BANCARIA",
        "numero_operacion": null,
        "productos": [{
            "nombre": "LECHE ENTERA", "cantidad": 1.0, "unidad": "unidad",
            "precio_unitario": total, "precio_total": total,
            "descuento": 0.0, "iva_porcentaje": 4.0, "iva_importe": 0.0
        }],
        "iva_desglose": [],
        "processing_profile": "fake",
        "warnings": []
    }))
    .into_response()
}

// ---------------------------------------------------------------------------
// Estado de la aplicación y llamadas HTTP
// ---------------------------------------------------------------------------

pub fn test_config() -> AppConfig {
    AppConfig {
        database_url: String::new(),
        jwt_secret: JWT_SECRET.into(),
        host: "127.0.0.1".into(),
        port: 0,
        intelligence_service_url: String::new(),
        intelligence_api_key: None,
        intelligence_timeout_secs: 5,
        intelligence_max_retries: 0,
        demo_user_email: Some("demo@example.com".into()),
        cors_origins: vec![],
        google: Some(GoogleConfig {
            client_id: "cliente-test".into(),
            client_secret: "secreto-test".into(),
            redirect_uri: "http://localhost:3000/api/auth/google/callback".into(),
            token_key: [9; 32],
        }),
        gmail_ticket_sender: DEFAULT_TICKET_SENDER.into(),
        gmail_sync_interval_minutes: 0,
    }
}

pub fn test_state(pool: PgPool, fake: &FakeGoogle) -> AppState {
    let config = test_config();
    let google =
        GoogleIntegration::new(config.google.as_ref().unwrap(), &config.gmail_ticket_sender)
            .unwrap()
            .with_fake_google(&fake.base_url);
    AppState {
        db_pool: pool,
        intelligence_client: IntelligenceClient::new(fake.base_url.clone(), None, 5, 0).unwrap(),
        google: Some(Arc::new(google)),
        config,
    }
}

pub fn app(state: &AppState) -> Router {
    Router::new()
        .nest("/api/auth", routes::auth_router(state.clone()))
        .nest("/api/gmail", routes::gmail_router(state.clone()))
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl Reply {
    pub fn location(&self) -> String {
        self.headers
            .get(header::LOCATION)
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default()
    }

    /// `nombre=valor` de la primera Set-Cookie, listo para reenviar en `Cookie`.
    pub fn cookie_pair(&self) -> Option<String> {
        let raw = self.headers.get(header::SET_COOKIE)?.to_str().ok()?;
        Some(raw.split(';').next()?.to_string())
    }

    pub fn set_cookie(&self) -> String {
        self.headers
            .get(header::SET_COOKIE)
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default()
    }

    pub fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or(Value::Null)
    }
}

pub async fn send(app: &Router, request: Request<Body>) -> Reply {
    let response = app.clone().oneshot(request).await.unwrap();
    let (parts, body) = response.into_parts();
    Reply {
        status: parts.status,
        headers: parts.headers,
        body: to_bytes(body, 1 << 20).await.unwrap().to_vec(),
    }
}

pub fn get_req(uri: &str, cookie: Option<&str>, bearer: Option<&str>) -> Request<Body> {
    let mut b = Request::builder().method("GET").uri(uri);
    if let Some(c) = cookie {
        b = b.header(header::COOKIE, c);
    }
    if let Some(t) = bearer {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    b.body(Body::empty()).unwrap()
}

pub fn authed(method: &str, uri: &str, bearer: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {bearer}"))
        .body(Body::empty())
        .unwrap()
}
