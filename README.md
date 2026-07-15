# Mercastats

[![Rust](https://img.shields.io/badge/backend-Rust-orange?logo=rust)](https://www.rust-lang.org/)
[![Leptos](https://img.shields.io/badge/frontend-Leptos%20%2B%20WASM-red)](https://leptos.dev/)
[![Python](https://img.shields.io/badge/intelligence-Python-blue?logo=python)](https://www.python.org/)
[![PostgreSQL](https://img.shields.io/badge/data-PostgreSQL-336791?logo=postgresql)](https://www.postgresql.org/)
[![Docker](https://img.shields.io/badge/infra-Docker-2496ED?logo=docker)](https://www.docker.com/)
[![License: MIT](https://img.shields.io/badge/license-MIT-green.svg)](LICENSE)

Mercastats convierte tickets de compra en un historial consultable de gasto, productos y hábitos de consumo. Es un monorepo full-stack con una API en Rust, un cliente Leptos compilado a WebAssembly, PostgreSQL y un servicio Python para extracción de tickets y predicciones.

## Demo pública

El despliegue previsto para la demo está en [mercastats.app](https://mercastats.app/). En la comprobación del 14 de julio de 2026 el servidor no respondió antes de agotar un tiempo de espera de 15 segundos, por lo que su disponibilidad no está garantizada. Si no carga, el proyecto puede ejecutarse localmente con Docker siguiendo las instrucciones de abajo.

| Usuario | Contraseña |
| --- | --- |
| `demo@demo.com` | `demodemo` |

Usa únicamente datos sintéticos o no sensibles. El repositorio no promete borrado automático de cuentas o tickets.

## Qué incluye

- Registro e inicio de sesión con contraseñas protegidas mediante bcrypt y sesiones JWT.
- Ingesta de tickets PDF e imagen, con normalización de productos y persistencia relacional.
- Dashboard de gasto, historial de tickets y estadísticas agregadas.
- Predicción experimental de próxima compra mediante un microservicio Python.
- Contenedores independientes y comprobaciones de salud para los servicios.

## Arquitectura

```mermaid
flowchart LR
    Browser["Leptos SPA / WebAssembly"] --> Nginx["Nginx"]
    Nginx -->|/api| API["Axum API / Rust"]
    API --> DB[(PostgreSQL)]
    API --> Intelligence["FastAPI intelligence service"]
    Intelligence --> OCR["PDF/image parsing"]
    Intelligence --> ML["purchase prediction"]
```

La separación mantiene la autenticación, la orquestación y el acceso tipado a datos en Rust, mientras reserva Python para OCR y experimentación con modelos. El cliente consume la API desde WebAssembly y Nginx actúa como servidor estático y proxy.

## Puesta en marcha con Docker

Requisitos: Docker Desktop y Docker Compose.

```bash
cp .env.example .env
openssl rand -base64 32
```

Edita `.env` y sustituye los valores `change_me` de `POSTGRES_PASSWORD` y `JWT_SECRET`. Después:

```bash
docker compose up --build
```

Servicios locales:

- Aplicación: <http://localhost:3000>
- API Rust: <http://localhost:8000/health>
- Servicio de inteligencia: <http://localhost:8001/health>

Para detener el stack:

```bash
docker compose down
```

Añade `--volumes` solo si también quieres eliminar la base de datos local.

## Desarrollo y comprobaciones

Backend Rust, con PostgreSQL levantado y el esquema aplicado:

```bash
docker compose up -d db
DATABASE_URL=postgres://mercastats_app:TU_PASSWORD@localhost:5432/mercastats \
  cargo test -p mercastats-backend
```

Frontend:

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd frontend
trunk serve
```

Servicio de inteligencia:

```bash
python3 -m venv .venv
source .venv/bin/activate
python -m pip install -r intelligence-service/requirements.txt pytest
python -m pytest intelligence-service/test_predictor.py
```

## Estructura

```text
backend/               API Axum, autenticación, SQLx e integración de tickets
frontend/              SPA Leptos/WASM y configuración de Nginx
intelligence-service/  Servicio FastAPI de OCR y predicción
ocr-service/           Prototipo independiente del parser de tickets
backend/migrations/    Esquema, funciones y vistas versionadas de PostgreSQL
docs/                  Decisiones, planes técnicos y notas de implementación
docker-compose.yml     Orquestación local del stack principal
```

## Limitaciones conocidas

- La extracción está especializada en formatos de Mercadona; no es un parser universal de recibos.
- Las predicciones son experimentales y dependen de disponer de historial suficiente.
- La configuración local no debe reutilizar secretos ni credenciales de producción.
- Antes de subir un ticket real, revisa su contenido: un recibo puede incluir datos de pago, localización y hábitos de compra.

## Autor

Creado por [Juan Carlos Negrín](https://github.com/Darkrai500).

## Licencia

Este proyecto se distribuye bajo la [licencia MIT](LICENSE).
