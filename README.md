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
- Inicio de sesión con Google y sincronización automática de los tickets digitales que Mercadona envía por correo (opcional, ver [más abajo](#inicio-de-sesión-con-google-y-sincronización-con-gmail)).
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

## Arranque rápido (macOS, Linux y Windows)

En macOS y Linux (Arch/CachyOS, Debian/Ubuntu, Fedora o openSUSE):

```bash
./run.sh
```

En Windows, desde PowerShell:

```powershell
.\run.ps1
```

El lanzador prepara lo que falte: `.env` con secretos aleatorios, Node.js, Rust y su objetivo WebAssembly, Trunk, Python 3.11, las dependencias de los servicios Python (inteligencia y prototipo OCR), Tesseract con español y Docker. Usa Homebrew en macOS, el gestor de paquetes de la distribución en Linux y winget en Windows. Algunas instalaciones del sistema pueden pedir privilegios de administrador; en Linux, el usuario debe tener acceso al socket de Docker. Después levanta PostgreSQL y lanza el servicio Python, la API Rust y el frontend con recarga automática. En macOS y Linux abre <http://localhost:3000> cuando responde el frontend; en Windows visita esa URL. La primera compilación puede tardar varios minutos.

En CachyOS/Arch agrupa los paquetes que falten en una sola instalación con `pacman` (incluidos `uv`, Trunk y `rust-wasm` si se usa el Rust del sistema). No ejecuta una actualización completa del sistema; si los mirrors devuelven errores 404, actualiza con `sudo pacman -Syu` y vuelve a lanzar el script.

| Comando | Qué hace |
| --- | --- |
| `./run.sh` | Desarrollo con recarga (BD en Docker + 3 procesos nativos) |
| `./run.sh docker` | Todo el stack en contenedores (`docker compose up --build`) |
| `./run.sh test` | Tests de Python y del backend Rust |
| `./run.sh setup` | Solo prepara el entorno |
| `./run.sh stop` | Detiene la base de datos (los datos se conservan) |

En Windows, usa los mismos modos con `.\run.ps1`, por ejemplo `.\run.ps1 setup` o `.\run.ps1 docker`. El modo `docker` solo requiere Docker y no instala las herramientas nativas. En WSL2, usa `./run.sh` con la integración de Docker Desktop activada.

Si el puerto 5432 ya lo usa otro PostgreSQL, el script usa automáticamente otro puerto libre para la base de datos de Mercastats y lo guarda en `.env`.

## Puesta en marcha con Docker

Requisitos para el arranque manual: Docker y Docker Compose v2. En Windows y macOS puedes usar Docker Desktop; el lanzador instala o prepara Docker cuando hace falta.

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

## Inicio de sesión con Google y sincronización con Gmail

Opcional. Con credenciales de Google configuradas, el login y el registro muestran el botón **Continuar con Google**. Un único consentimiento da identidad y acceso de **solo lectura** a Gmail; a partir de ahí Mercastats:

1. busca en la bandeja los correos de `ticket_digital@mail.mercadona.com` con un PDF adjunto,
2. pasa cada PDF por el mismo OCR y la misma ingesta que la subida manual,
3. lo repite solo cada 30 minutos (y al pulsar **Sincronizar ahora** en «Subir tickets»).

Cada correo se procesa una sola vez, y un ticket que ya está en la base de datos (subido a mano o importado antes) se cuenta como duplicado en lugar de repetirse. Sin `GOOGLE_CLIENT_ID` ni `GOOGLE_CLIENT_SECRET` la función queda desactivada y el botón no aparece.

### Configuración en Google Cloud

1. En [Google Cloud Console](https://console.cloud.google.com/) crea un proyecto y habilita la **Gmail API** (APIs y servicios → Biblioteca).
2. En **Pantalla de consentimiento de OAuth** elige *Externo*, rellena el nombre y el correo de contacto, añade el ámbito `https://www.googleapis.com/auth/gmail.readonly` (además de `openid`, `email` y `profile`) y, mientras la app esté en modo *Pruebas*, añade tu correo como **usuario de prueba**.
3. En **Credenciales** crea un *ID de cliente de OAuth* de tipo *Aplicación web* con la URI de redireccionamiento autorizada `http://localhost:3000/api/auth/google/callback` (en un despliegue real, la URL HTTPS equivalente del frontend).
4. Copia el ID y el secreto a `.env` (`GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET`) y relanza `./run.sh`. `TOKEN_ENCRYPTION_KEY` la genera el propio script si falta.

| Variable | Para qué sirve |
| --- | --- |
| `GOOGLE_CLIENT_ID`, `GOOGLE_CLIENT_SECRET` | Credenciales OAuth. Ambas o ninguna. |
| `GOOGLE_REDIRECT_URI` | Debe coincidir exactamente con la registrada en Google (por defecto, la de `localhost:3000`). |
| `TOKEN_ENCRYPTION_KEY` | Clave AES-256 (32 bytes en base64) que cifra en la base de datos el token de acceso permanente. Si se cambia, hay que volver a conectar Gmail. |
| `GMAIL_TICKET_SENDER` | Remitente cuyos correos se importan. |
| `GMAIL_SYNC_INTERVAL_MINUTES` | Cada cuánto se sincroniza (`0` = solo a mano). |

### Qué conviene saber

- **`gmail.readonly` es un ámbito restringido de Google.** En modo *Pruebas* solo pueden entrar los usuarios de prueba que añadas (máximo 100), aparece el aviso de «app no verificada» y, según la documentación de Google, **el token de acceso permanente caduca a los 7 días**: Mercastats lo detecta y muestra «Requiere reconexión». Pasar el proyecto a *En producción* evita esa caducidad semanal para uso personal; abrirlo a usuarios generales exige que Google verifique la aplicación (y, al guardar datos de Gmail en un servidor, una evaluación de seguridad externa periódica).
- **Qué se guarda:** el token cifrado, el PDF de cada ticket (igual que en la subida manual) y los identificadores de los correos ya procesados. No se almacenan asuntos ni cuerpos de correo. El permiso es de solo lectura, pero Google no ofrece uno más estrecho que «solo los correos de un remitente»: esa restricción la aplica Mercastats en su búsqueda.
- **Cuentas:** entrar con Google crea una cuenta sin contraseña. Si ya existe una cuenta con contraseña para ese email, Google **no se enlaza solo** (el registro con contraseña no verifica el correo): inicia sesión con tu contraseña y pulsa **Conectar Gmail**. El Gmail conectado no tiene que coincidir con el email de la cuenta.
- **Desconectar** revoca el acceso en Google y borra el token; los tickets ya importados se conservan.
- **Problemas frecuentes:** `redirect_uri_mismatch` → la URI de Google Cloud no coincide con `GOOGLE_REDIRECT_URI`; «Acceso bloqueado: la app no ha completado la verificación» → falta tu correo como usuario de prueba; «Gmail denegó el acceso» → la Gmail API no está habilitada en el proyecto.

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
- La sincronización con Gmail asume que el ticket llega como PDF adjunto desde la dirección configurada. Sus pruebas automáticas usan un Gmail simulado con datos sintéticos; no se han contrastado con correos reales de Mercadona.
- La predicción necesita al menos 5 días con compra para mostrarse y su confianza es deliberadamente conservadora con historiales cortos. No modela festivos ni cambios de hábito recientes.
- La configuración local no debe reutilizar secretos ni credenciales de producción.
- Antes de subir un ticket real, revisa su contenido: un recibo puede incluir datos de pago, localización y hábitos de compra.

## Autor

Creado por [Juan Carlos Negrín](https://github.com/Darkrai500).

## Licencia

Este proyecto se distribuye bajo la [licencia MIT](LICENSE).
