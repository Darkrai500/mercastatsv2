# 📊 Mercastats

> **Análisis inteligente de tickets y hábitos de compra en Mercadona**

[![Rust](https://img.shields.io/badge/Rust-1.77+-orange.svg)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-blue.svg)](https://www.postgresql.org/)
[![FastAPI](https://img.shields.io/badge/FastAPI-Intelligence%20Service-009485.svg)](https://fastapi.tiangolo.com/)
[![Status](https://img.shields.io/badge/Status-En%20Desarrollo-yellow.svg)](https://github.com/tu-usuario/mercastats)

---

## 🎯 ¿Qué es Mercastats?

Mercastats es una plataforma full-stack que conecta **backend en Rust**, **servicio de inteligencia en Python (OCR + ML)** y **frontend Leptos** para digitalizar tus tickets de Mercadona y generar insights accionables. Con ella puedes:

- 📸 **Procesar tickets PDF** con OCR y guardarlos en PostgreSQL con validaciones.
- 📜 **Consultar el historial completo** de tickets por usuario.
- 📊 **Explorar estadísticas**: tendencia diaria, comparativa mensual, distribución semanal/horaria y productos top.
- 🤖 **Recibir predicciones** sobre tu próxima compra y sugerencias basadas en tu histórico.
- 🖥️ **Usar un frontend reactivo** construido en Rust (WASM) + Tailwind.

---

## ✨ Características Principales

### Implementado ✅
- ✅ **Autenticación con JWT**: Registro, login y middleware de autorización.
- ✅ **Procesamiento OCR**: Endpoint `/api/ocr/process` que llama al **Intelligence Service** (FastAPI) para extraer factura, fecha, total, desglose de IVA y líneas de producto.
- ✅ **Ingesta de tickets**: Validaciones, idempotencia por número de factura y escritura transaccional en `usuarios`, `compras`, `compras_productos` y PDFs.
- ✅ **Historial de tickets**: Endpoint `/api/tickets/history` con paginación y métricas agregadas por usuario.
- ✅ **Dashboard de estadísticas**: `/api/stats/dashboard` y `/api/stats/monthly` con tendencia diaria, comparación mes actual vs anterior, top productos por cantidad/gasto y distribuciones semanal/horaria.
- ✅ **Predicción de próxima compra**: `/api/predict/next` combina vistas analíticas (`ml_ticket_features`) con el modelo Python para estimar ventana temporal, total esperado y productos sugeridos.
- ✅ **Frontend Leptos + Tailwind**: Páginas de login/registro, subida de tickets, historial, dashboard, evolución mensual y predicción.

### En el radar 🔍
- 🔎 Mejora de OCR (afinado de parsing y warm-up en despliegue).
- 🔎 Gráficos avanzados en frontend (Chart.js/Plotters) y comparativas de tiendas.
- 🔎 Gamificación (objetivos, logros) y refresco de tokens.
- 🔎 Dockerización completa y healthchecks unificados.

---

## 🏗️ Arquitectura Técnica

```
┌────────────────────────────────────────────────────────┐
│                FRONTEND (Leptos + Tailwind)            │
│  - SPA WASM: login, registro, upload, historial,       │
│    dashboard, evolución mensual, predicciones          │
└───────────────────────────┬────────────────────────────┘
                            │ (REST + JWT)
                            ▼
┌────────────────────────────────────────────────────────┐
│            BACKEND (Rust · Axum · SQLx)                │
│  - Auth JWT, middleware y validaciones                 │
│  - OCR + ingesta de tickets                            │
│  - Estadísticas (tendencias, top productos,            │
│    distribuciones)                                     │
│  - Orquestación con Intelligence Service               │
└─────────────┬──────────────────────┬──────────────────┘
              │                      │
              ▼                      ▼
      PostgreSQL 16          Intelligence Service (FastAPI)
      - Schema completo      - /ocr/process (pdfplumber)
      - Vistas analíticas    - /predict/next (scikit-learn)
      - Índices y checks     - /health
```

| Componente | Tecnología | Por qué |
|------------|-----------|---------|
| **Backend** | Rust + Axum | Rendimiento, seguridad de memoria y tipado fuerte. |
| **Database** | PostgreSQL 16 | Funciones analíticas, vistas para ML y constraints sólidos. |
| **ORM** | SQLx | Validación de consultas en compile-time. |
| **Intelligence** | FastAPI + pdfplumber + scikit-learn | OCR robusto y modelos de predicción reutilizables. |
| **Frontend** | Leptos (WASM) + Tailwind | UI reactiva en Rust con estilo utility-first. |

---

## 🚀 Quick Start

### Prerrequisitos

```powershell
# Rust toolchain
rustup --version  # 1.77+

# PostgreSQL
psql --version    # 16+

# Python para Intelligence Service (OCR + ML)
python3 --version # 3.11+ recomendado

# Herramientas adicionales
cargo install sqlx-cli --no-default-features --features postgres
cargo install trunk               # Frontend Leptos
npm install -g pm2 (opcional)     # Orquestación alternativa
```

### Instalación y arranque

```powershell
# 1) Clonar y preparar entorno
git clone https://github.com/tu-usuario/mercastats.git
cd mercastats
cp .env.example .env   # Ajusta DATABASE_URL, JWT_SECRET e INTELLIGENCE_SERVICE_URL

# 2) Base de datos
psql -U postgres -c "CREATE DATABASE mercastats;"
psql -U postgres -d mercastats -f sql/schema/schema.sql

# 3) Servicio de inteligencia (FastAPI)
cd intelligence-service
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt
uvicorn main:app --host 127.0.0.1 --port 8001 --reload &
cd ..

# 4) Backend y frontend
cargo sqlx prepare --workspace   # valida queries
node dev.js                      # levanta intelligence + backend + frontend
# Flags útiles: --backend-only | --frontend-only | --intelligence-only | --release
```

Estructura rápida del repo:

```
backend/                # Axum + SQLx (auth, OCR, stats, predicciones)
frontend/               # Leptos + Tailwind (pages: login, registro, upload, historial, dashboard, monthly, prediction)
intelligence-service/   # FastAPI (OCR + ML)
sql/schema/             # Schema SQL completo + vistas analíticas
docs/                   # Documentación técnica y planes
dev.js                  # Orquestador local (Node)
```

---

## 🧪 Testing

```powershell
# Ejecutar tests de Rust (workspace)
cargo test --workspace

# Backend con logs detallados
cd backend
cargo test -- --nocapture
```

Para validar consultas SQL, ejecuta `cargo sqlx prepare --workspace` tras modificar queries.

---

## 📚 Documentación

- [claude.md](../claude.md)
- [docs/BACKEND_TICKET_INGESTION_PLAN.md](BACKEND_TICKET_INGESTION_PLAN.md)
- [docs/OCR_INTEGRATION_NOTES.md](OCR_INTEGRATION_NOTES.md)
- [docs/OCR_WARMUP_IMPLEMENTATION.md](OCR_WARMUP_IMPLEMENTATION.md)
- [docs/WARMUP_FEATURE_SUMMARY.md](WARMUP_FEATURE_SUMMARY.md)
- [sql/schema/schema.sql](../sql/schema/schema.sql)
- [frontend/README.md](../frontend/README.md)

Recursos externos: [Rust Book](https://doc.rust-lang.org/book/), [Axum](https://docs.rs/axum/), [SQLx](https://github.com/launchbadge/sqlx), [Leptos](https://leptos-rs.github.io/leptos/), [FastAPI](https://fastapi.tiangolo.com/).

---

## 🛣️ Roadmap

### Implementado ✅
- Setup del proyecto y base de datos con vistas para ML (`ml_ticket_features`).
- Backend core (auth, middleware, validaciones, ingestión de tickets, stats, predicciones).
- Integración completa con Intelligence Service (OCR + predict).
- Frontend Leptos con páginas principales y consumo de API.

### Próximos pasos 📋
- [ ] Gráficos y visualizaciones avanzadas en el dashboard.
- [ ] Gamificación (logros, objetivos) y refresco de tokens.
- [ ] Paquetes Docker y healthchecks unificados (backend + intelligence).
- [ ] Suite de tests end-to-end y contract tests para OCR/ML.

---

## 📄 Licencia

MIT. Consulta `LICENSE` para más detalles.

---

## 👨‍💻 Autor

**Juan Carlos**

---

## 📊 Estado del Proyecto

```
Progreso General: ██████████████████░░░░ 80%
Backend:          ████████████████████░░ 85%
Frontend:         ███████████████░░░░░░░ 70%
Intelligence:     ██████████████░░░░░░░░ 65%
Documentación:    █████████████████░░░░ 85%
Tests:            █████████░░░░░░░░░░░░░ 55%
```

<p align="center">
Hecho con ❤️ y 🦀 (Rust)
</p>
