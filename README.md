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
- Predicción de la próxima compra (día y hora, gasto estimado y productos que toca reponer) mediante un modelo estadístico por usuario en el microservicio Python.
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

## Arranque rápido (macOS)

```bash
./run.sh
```

Un solo comando prepara lo que falte (`.env` con secretos aleatorios, entorno virtual de Python, objetivo `wasm32` de Rust y `trunk`), arranca el motor de Docker (OrbStack o Docker Desktop) si está apagado, levanta PostgreSQL y lanza el servicio Python, la API Rust y el frontend con recarga automática. Cuando el frontend termina de compilar abre <http://localhost:3000>. La primera vez tarda unos minutos; las siguientes, segundos.

| Comando | Qué hace |
| --- | --- |
| `./run.sh` | Desarrollo con recarga (BD en Docker + 3 procesos nativos) |
| `./run.sh docker` | Todo el stack en contenedores (`docker compose up --build`) |
| `./run.sh test` | Tests de Python y del backend Rust |
| `./run.sh setup` | Solo prepara el entorno |
| `./run.sh stop` | Detiene la base de datos (los datos se conservan) |

Si el puerto 5432 ya lo usa otro PostgreSQL (por ejemplo, uno instalado en el Mac), el script usa automáticamente otro puerto libre para la base de datos de Mercastats y lo guarda en `.env`.

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
python -m pip install -r intelligence-service/requirements.txt pytest httpx
python -m pytest intelligence-service
```

Para evaluar el predictor con usuarios sintéticos (precisión del día, error de hora y gasto, calidad de las sugerencias de productos):

```bash
cd intelligence-service && python backtest.py
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
- La predicción necesita al menos 5 días con compra para mostrarse y su confianza es deliberadamente conservadora con historiales cortos. No modela festivos ni cambios de hábito recientes.
- La configuración local no debe reutilizar secretos ni credenciales de producción.
- Antes de subir un ticket real, revisa su contenido: un recibo puede incluir datos de pago, localización y hábitos de compra.

## Autor

Creado por [Juan Carlos Negrín](https://github.com/Darkrai500).

## Licencia

Este proyecto se distribuye bajo la [licencia MIT](LICENSE).
