# 🤖 Guía de Desarrollo para Claude Code - Mercastats

> **Documento de referencia para el agente Claude Code**  
> Última actualización: 24 de octubre de 2025

---

## 📋 Tabla de Contenidos

1. [Visión General del Proyecto](#-visión-general-del-proyecto)
2. [Arquitectura del Sistema](#-arquitectura-del-sistema)
3. [Estructura del Proyecto](#-estructura-del-proyecto)
4. [Configuración del Entorno](#-configuración-del-entorno)
5. [Guías de Desarrollo](#-guías-de-desarrollo)
6. [Convenciones de Código](#-convenciones-de-código)
7. [Base de Datos](#-base-de-datos)
8. [Testing](#-testing)
9. [Comandos Útiles](#-comandos-útiles)
10. [Troubleshooting](#-troubleshooting)
11. [Recursos y Referencias](#-recursos-y-referencias)

---

## 🎯 Visión General del Proyecto

**Mercastats** es una aplicación web full-stack para análisis estadístico de compras del supermercado Mercadona. Permite a los usuarios:

- 📄 Subir tickets de compra (PDF/imágenes)
- 📊 Visualizar estadísticas de gasto
- 📈 Analizar tendencias de consumo
- 💰 Calcular inflación personal
- 🎯 Establecer objetivos de ahorro
- 🏆 Desbloquear logros (gamificación)

### 🎓 Objetivos Educativos

Este proyecto es una **oportunidad de aprendizaje** que combina:

- ✅ Rust avanzado (ownership, async/await, traits)
- ✅ WebAssembly (Leptos frontend)
- ✅ PostgreSQL (queries complejas, triggers, vistas)
- ✅ Arquitectura de microservicios
- ✅ Machine Learning (Python workers)
- ✅ OCR y procesamiento de documentos

---

## 🏗️ Arquitectura del Sistema

### Visión de Alto Nivel

```
┌─────────────────────────────────────────────────┐
│           FRONTEND (Leptos - WASM)             │
│  - SPA reactivo                                 │
│  - Gráficos interactivos (Chart.js)            │
│  - Tailwind CSS                                 │
└────────────────┬────────────────────────────────┘
                 │ REST API (HTTP/JSON)
                 ↓
┌─────────────────────────────────────────────────┐
│        BACKEND (Rust - Axum/SQLx)              │
│  - API REST endpoints                           │
│  - Lógica de negocio                           │
│  - Autenticación JWT                           │
│  - Validación de datos                         │
└─────┬───────────────┬─────────────────┬────────┘
      │               │                 │
      ↓               ↓                 ↓
┌──────────┐  ┌─────────────┐  ┌──────────────┐
│PostgreSQL│  │OCR Service  │  │ ML Service   │
│   16+    │  │  (Python)   │  │  (Python)    │
│          │  │  FastAPI    │  │  FastAPI     │
│ - Datos  │  │  Tesseract  │  │  scikit-learn│
│ - Vistas │  └─────────────┘  └──────────────┘
│ - Triggers│
└──────────┘
```

### 🔑 Decisiones Técnicas Clave

| Componente   | Tecnología            | Justificación                                       |
| ------------ | --------------------- | --------------------------------------------------- |
| **Backend**  | Rust + Axum           | Rendimiento extremo, type-safety, async nativo      |
| **Database** | PostgreSQL 16         | Funciones analíticas avanzadas, JSON, triggers      |
| **ORM**      | SQLx                  | Compile-time query validation, sin runtime overhead |
| **Frontend** | Leptos (WASM)         | Rendimiento cercano a nativo, Rust end-to-end       |
| **OCR**      | Python + Tesseract    | Ecosistema maduro para visión por computadora       |
| **ML**       | Python + scikit-learn | Facilidad para prototipado y experimentación        |

---

## 📁 Estructura del Proyecto

```
mercastats/
├── 📦 Cargo.toml                    # Workspace principal
├── 📦 Cargo.lock                    # Lock de dependencias
├── 🔐 .env                          # Variables de entorno (NO versionar)
├── 📝 .gitignore
├── 📚 claude.md                     # Este documento
│
├── 📂 backend/                      # Backend en Rust
│   ├── 📦 Cargo.toml
│   ├── 🔐 .env                      # Configuración específica
│   ├── 📂 src/
│   │   ├── 🦀 main.rs              # Punto de entrada
│   │   ├── 🦀 config.rs            # Configuración de la app
│   │   ├── 🦀 error.rs             # Manejo de errores
│   │   ├── 📂 models/              # Modelos de dominio
│   │   ├── 📂 schema/              # DTOs y validación
│   │   ├── 📂 db/                  # Capa de acceso a datos
│   │   ├── 📂 services/            # Lógica de negocio
│   │   ├── 📂 routes/              # Endpoints HTTP
│   │   ├── 📂 middleware/          # Middleware personalizado
│   │   └── 📂 utils/               # Utilidades
│   ├── 📂 migrations/              # Migraciones SQLx
│   └── 📂 tests/                   # Tests de integración
│
├── 📂 frontend/                     # Frontend Leptos (futuro)
│   ├── 📦 Cargo.toml
│   ├── 🎨 style/                   # Tailwind CSS
│   └── 📂 src/
│       ├── 🦀 main.rs
│       ├── 📂 components/          # Componentes reutilizables
│       ├── 📂 pages/               # Páginas/vistas
│       └── 📂 api/                 # Cliente API
│
├── 📂 ocr-service/                  # Worker Python OCR (futuro)
│   ├── 🐍 requirements.txt
│   ├── 🐳 Dockerfile
│   └── 📂 src/
│
├── 📂 ml-service/                   # Worker Python ML (futuro)
│   ├── 🐍 requirements.txt
│   ├── 🐳 Dockerfile
│   └── 📂 src/
│
├── 📂 sql/                          # Scripts SQL
│   └── 📂 schema/
│       └── 📜 schema.sql           # Schema completo de PostgreSQL
│
└── 📂 docs/                         # Documentación (referencia)
    ├── 📄 mercadona_stats_ideas.md
    ├── 📄 MERCASTATS_TECH_STACK.md
    ├── 📄 MERCASTATS_SCHEMA_GUIDE.md
    └── 📄 mercastats_schema.sql
```

---

## ⚙️ Configuración del Entorno

### Prerrequisitos

```powershell
# Verificar instalaciones
rustc --version          # Rust 1.75+
cargo --version          # Cargo (incluido con Rust)
psql --version           # PostgreSQL 16+
node --version           # Node.js 20+ (para frontend)

# Herramientas adicionales
cargo install sqlx-cli --no-default-features --features postgres
cargo install cargo-watch
cargo install trunk      # Para frontend Leptos
```

### 🗄️ Setup de Base de Datos

```powershell
# 1. Crear base de datos
psql -U postgres
# En psql:
CREATE DATABASE mercastats;
CREATE USER mercastats_app WITH PASSWORD 'MercaStats2025!';
GRANT ALL PRIVILEGES ON DATABASE mercastats TO mercastats_app;
\q

# 2. Ejecutar schema inicial
psql -U postgres -d mercastats -f sql/schema/schema.sql

# 3. Verificar tablas creadas
psql -U mercastats_app -d mercastats -c "\dt"
```

### 🔐 Variables de Entorno

**Archivo `.env` en la raíz:**

```env
DATABASE_URL=postgres://mercastats_app:MercaStats2025!@localhost:5432/mercastats
RUST_LOG=debug,mercastats_backend=debug,sqlx=info
JWT_SECRET=K7gNU3sdo+OL0wNhqoVWhr3g6s1xYv72ol/pe/Unols=
BACKEND_HOST=127.0.0.1
BACKEND_PORT=8000
```

**⚠️ IMPORTANTE**: Nunca versionar el archivo `.env` - contiene secretos

---

## 🛠️ Guías de Desarrollo

### 🚀 Inicio Rápido

```powershell
# 1. Clonar y setup
cd C:\Users\jcneg\Documents\mercastats

# 2. Instalar dependencias
cargo build

# 3. Ejecutar backend en modo desarrollo
cd backend
cargo watch -x run

# En otra terminal - ejecutar tests
cargo test

# Ver documentación generada
cargo doc --open
```

### 📝 Crear un Nuevo Endpoint

**Ejemplo: Crear endpoint GET /api/health**

1. **Definir el handler en `routes/health.rs`:**

```rust
use axum::{Json, response::IntoResponse};
use serde::Serialize;

#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    timestamp: i64,
    database: String,
}

pub async fn health_check() -> impl IntoResponse {
    let response = HealthResponse {
        status: "ok".to_string(),
        timestamp: chrono::Utc::now().timestamp(),
        database: "connected".to_string(),
    };

    Json(response)
}
```

2. **Registrar la ruta en `main.rs`:**

```rust
use axum::{Router, routing::get};

let app = Router::new()
    .route("/api/health", get(health_check));
```

3. **Probar el endpoint:**

```powershell
curl http://localhost:8000/api/health
```

### 🗃️ Interactuar con la Base de Datos

**Ejemplo: Query con SQLx**

```rust
use sqlx::PgPool;

pub async fn get_user_by_email(
    pool: &PgPool,
    email: &str
) -> Result<User, sqlx::Error> {
    sqlx::query_as!(
        User,
        "SELECT email, nombre, created_at FROM usuarios WHERE email = $1",
        email
    )
    .fetch_one(pool)
    .await
}
```

**✨ SQLx valida las queries en COMPILE-TIME**

Para que funcione, necesitas:

```powershell
# Exportar DATABASE_URL
$env:DATABASE_URL="postgres://mercastats_app:MercaStats2025!@localhost:5432/mercastats"

# Ejecutar en el directorio del proyecto
cargo sqlx prepare

# Esto crea .sqlx/ con metadata offline
```

### 🔒 Autenticación JWT

**Crear y verificar tokens:**

```rust
use jsonwebtoken::{encode, decode, Header, Validation, EncodingKey, DecodingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,  // email
    exp: usize,   // expiration
}

// Crear token
let claims = Claims {
    sub: user.email.clone(),
    exp: (Utc::now() + Duration::hours(24)).timestamp() as usize,
};

let token = encode(
    &Header::default(),
    &claims,
    &EncodingKey::from_secret(jwt_secret.as_bytes())
)?;

// Verificar token
let token_data = decode::<Claims>(
    &token,
    &DecodingKey::from_secret(jwt_secret.as_bytes()),
    &Validation::default()
)?;
```

---

## 📏 Convenciones de Código

### 🦀 Estilo Rust

Seguimos la [Rust Style Guide](https://doc.rust-lang.org/beta/style-guide/index.html) oficial:

```




---

## 🗄️ Base de Datos

### 📊 Schema Completo

El schema SQL completo está en `sql/schema/schema.sql`. Tablas principales:

1. **usuarios** - Usuarios registrados (PK: email)
2. **productos** - Catálogo de productos (PK: nombre)
3. **compras** - Tickets de compra (PK: numero_factura)
4. **compras_productos** - Relación M:N (PK: compuesta)
5. **historico_precios** - Histórico para inflación
6. **tickets_pdf** - PDFs almacenados (separado por rendimiento)
7. **logros** / **logros_usuario** - Sistema de gamificación
8. **objetivos_ahorro** - Metas mensuales
9. **preferencias_usuario** - Configuración personalizada



---

## 🧪 Testing

### Estructura de Tests

```

backend/tests/
├── integration/
│ ├── api_tests.rs # Tests de endpoints
│ ├── db_tests.rs # Tests de base de datos
│ └── auth_tests.rs # Tests de autenticación
└── common/
└── mod.rs # Utilidades compartidas

````



### 🗄️ Base de Datos

```powershell
# Conectar a PostgreSQL
psql -U mercastats_app -d mercastats

# Ejecutar query desde PowerShell
psql -U mercastats_app -d mercastats -c "SELECT * FROM usuarios LIMIT 5;"

# Ejecutar script SQL
psql -U mercastats_app -d mercastats -f script.sql

# Backup de base de datos
pg_dump -U postgres mercastats > backup.sql

# Restaurar backup
psql -U postgres -d mercastats < backup.sql

# Ver tamaño de tablas
psql -U mercastats_app -d mercastats -c "
SELECT
    schemaname,
    tablename,
    pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) AS size
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;"
````

### 🐳 Docker (futuro)

```powershell
# Construir imagen
docker build -t mercastats-backend .

# Ejecutar contenedor
docker run -p 8000:8000 --env-file .env mercastats-backend

# Docker Compose (todos los servicios)
docker-compose up -d

# Ver logs
docker-compose logs -f backend

# Parar servicios
docker-compose down
```

---

## 📚 Recursos y Referencias

### 📖 Documentación del Proyecto

| Archivo                         | Descripción                           |
| ------------------------------- | ------------------------------------- |
| `mercadona_stats_ideas.md`      | Lista de funcionalidades planificadas |
| `MERCASTATS_TECH_STACK.md`      | Especificación técnica completa       |
| `MERCASTATS_SCHEMA_GUIDE.md`    | Guía detallada del schema de BD       |
| `mercastats_schema.sql`         | Script SQL del schema completo        |
| `Mermaid-Modelo_relacional.txt` | Diagrama ER en formato Mermaid        |

### 🦀 Rust

- [The Rust Book](https://doc.rust-lang.org/book/) - Libro oficial
- [Rust by Example](https://doc.rust-lang.org/rust-by-example/) - Ejemplos prácticos
- [Async Book](https://rust-lang.github.io/async-book/) - Programación asíncrona
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) - Mejores prácticas

### 🌐 Frameworks y Librerías

- [Axum](https://docs.rs/axum/) - Framework web
- [SQLx](https://github.com/launchbadge/sqlx) - Async SQL toolkit
- [Tokio](https://tokio.rs/) - Async runtime
- [Serde](https://serde.rs/) - Serialización
- [Leptos](https://leptos-rs.github.io/leptos/) - Frontend WASM

### 🗄️ PostgreSQL

- [PostgreSQL Docs](https://www.postgresql.org/docs/16/)
- [Window Functions Tutorial](https://www.postgresql.org/docs/16/tutorial-window.html)
- [Trigger Tutorial](https://www.postgresql.org/docs/16/trigger-definition.html)

### 🎓 Tutoriales y Guías

- [Rust + Axum Tutorial](https://github.com/tokio-rs/axum/tree/main/examples)
- [SQLx Tutorial](https://www.sea-ql.org/blog/2022-04-04-getting-started-with-sqlx/)
- [WebAssembly with Rust](https://rustwasm.github.io/docs/book/)

---

## 🚀 Roadmap de Desarrollo

### ✅ Fase 1: MVP Backend (Actual)

- [x] Setup del proyecto (Cargo workspace)
- [x] Schema de base de datos completo
- [x] Configuración de variables de entorno
- [ ] **SIGUIENTE**: Estructura de módulos del backend
- [ ] Endpoint de health check
- [ ] Sistema de logging (tracing)
- [ ] Manejo centralizado de errores
- [ ] Pool de conexiones a PostgreSQL

### 📋 Fase 2: CRUD Básico

- [ ] Modelos de dominio (User, Producto, Compra)
- [ ] DTOs con validación
- [ ] Endpoints CRUD de usuarios
- [ ] Endpoints CRUD de compras
- [ ] Tests de integración básicos

### 🔐 Fase 3: Autenticación

- [ ] Registro de usuarios (hash de passwords)
- [ ] Login con JWT
- [ ] Middleware de autenticación
- [ ] Refresh tokens

### 📊 Fase 4: Estadísticas

- [ ] Endpoint de gasto medio
- [ ] Endpoint de productos más comprados
- [ ] Endpoint de evolución mensual
- [ ] Endpoint de distribución por categorías

### 🤖 Fase 5: Workers Python

- [ ] OCR Service (Tesseract)
- [ ] Parsing de tickets Mercadona
- [ ] ML Service (predicciones)
- [ ] Comunicación backend ↔ workers

### 🎨 Fase 6: Frontend Leptos

- [ ] Setup de Leptos
- [ ] Componentes básicos
- [ ] Dashboard principal
- [ ] Gráficos interactivos
- [ ] Upload de tickets

---

## 🎯 Mejores Prácticas de Claude Code

### ✅ Cuando Claude Code trabaja en este proyecto, debe:

1. **Leer primero la documentación relevante**:

   - Para queries de BD → `MERCASTATS_SCHEMA_GUIDE.md`
   - Para arquitectura → `MERCASTATS_TECH_STACK.md`
   - Para funcionalidades → `mercadona_stats_ideas.md`

2. **Seguir las convenciones establecidas**:

   - Rust idiomático (snake_case, Result, Option)
   - Documentación con `///` para funciones públicas
   - Tests en archivos `_tests.rs` o carpeta `tests/`

3. **Validar queries SQL**:

   - Usar `sqlx::query_as!` con tipos explícitos
   - Ejecutar `cargo sqlx prepare` después de cambios
   - Verificar índices para queries complejas

4. **Manejar errores correctamente**:

   - Usar `?` para propagación
   - Convertir errores con `thiserror`
   - Proveer contexto con `anyhow`

5. **Escribir tests**:

   - Test unitarios para lógica de negocio
   - Tests de integración para endpoints
   - Setup y cleanup de datos de prueba

6. **Documentar decisiones técnicas**:
   - Comentarios para lógica compleja
   - README en módulos nuevos
   - Actualizar este documento si hay cambios arquitectónicos

---

## 📝 Notas Finales

Este proyecto está en **fase inicial de desarrollo**. El objetivo es construir una aplicación full-stack moderna que sirva como:

- 🎓 **Plataforma de aprendizaje** de tecnologías avanzadas
- 💼 **Portfolio profesional** demostrando habilidades reales
- 🚀 **Base sólida** para expansión futura (mobile, API pública, etc.)

**Claude Code**, tu misión es ayudar a construir un backend robusto, mantenible y bien documentado siguiendo las mejores prácticas de la industria. ¡Adelante! 🦀

---

**Última actualización**: 24 de octubre de 2025  
**Versión**: 1.0  
**Autor**: Juan Carlos  
**Licencia**: MIT (pendiente)
