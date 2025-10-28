# 📊 Mercastats

> **Análisis estadístico inteligente de tus compras del Mercadona**

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-blue.svg)](https://www.postgresql.org/)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/Status-En%20Desarrollo-yellow.svg)](https://github.com/tu-usuario/mercastats)

---

## 🎯 ¿Qué es Mercastats?

Mercastats es una aplicación web full-stack que te permite:

- 📸 **Subir tickets** de compra (PDF)
- 📊 **Visualizar estadísticas** básicas de tus hábitos de consumo
- 📜 **Consultar tu historial** de tickets
- (_Próximamente_) 💰 Calcular tu inflación personal basada en tus productos favoritos
- (_Próximamente_) 📈 Detectar tendencias en tus compras
- (_Próximamente_) 🎯 Establecer objetivos de ahorro mensuales
- (_Próximamente_) 🏆 Desbloquear logros mientras haces tus compras más inteligentes

---

## ✨ Características Principales

### Implementado ✅
- ✅ **Autenticación de Usuarios**: Registro e inicio de sesión seguros con JWT.
- ✅ **Subida de Tickets**: Interfaz para subir archivos PDF de tickets de Mercadona.
- ✅ **Procesamiento OCR**: Extracción de datos (número de factura, fecha, total, productos) de PDFs usando una integración Python (PyO3).
- ✅ **Persistencia de Datos**: Guardado de la información de compras y productos en base de datos PostgreSQL.
- ✅ **Historial de Tickets**: Visualización del listado de tickets subidos con opción de ordenación.
- ✅ **Estadísticas Básicas**: Resumen de número total de tickets, gasto total y gasto promedio.
- ✅ **Frontend Reactivo**: Interfaz de usuario moderna construida con Leptos (WASM) y Tailwind CSS.

### 🔍 Análisis Avanzados (Futuro)
- 🔎 Detección de tendencias de consumo
- 🔎 Cálculo de inflación personalizada
- 🔎 Comparativa de ticket medio por tienda
- 🔎 Predicción de gasto del próximo mes

### 🎮 Gamificación (Futuro)
- 🏆 Sistema de logros desbloqueables
- 🎯 Objetivos de ahorro configurables
- 📅 Calendario de compras
- 🔥 Rachas y estadísticas personales

---

## 🏗️ Arquitectura Técnica

### Stack Tecnológico

| Componente | Tecnología | Por qué |
|------------|-----------|---------|
| **Backend** | Rust + Axum | Rendimiento extremo, type-safety, memoria segura |
| **Database** | PostgreSQL 16 | Funciones analíticas, JSON, triggers automáticos |
| **ORM** | SQLx | Validación de queries en compile-time |
| **Frontend** | Leptos (WASM) | Rendimiento nativo, Rust end-to-end |
| **OCR** | Python + pdfplumber | Lógica de extracción de texto y parsing integrada vía PyO3 |
| **ML** | Python + scikit-learn (_Futuro_) | Prototipado rápido de modelos predictivos |

---

## 🚀 Quick Start

### Prerrequisitos

```powershell
# Instalar Rust (https://rustup.rs/)
rustup --version  # 1.75+

# Instalar PostgreSQL (https://www.postgresql.org/download/)
psql --version    # 16+

# Instalar Python (necesario para la integración OCR vía PyO3)
python --version # 3.8+

# Herramientas adicionales
cargo install sqlx-cli --no-default-features --features postgres
cargo install cargo-watch
cargo install trunk # Para el frontend Leptos
```

### Instalación

```powershell
# 1. Clonar el repositorio
git clone https://github.com/tu-usuario/mercastats.git
cd mercastats

# 2. Configurar base de datos
psql -U postgres
CREATE DATABASE mercastats;
CREATE USER mercastats_app WITH PASSWORD 'tu_password'; # Cambia 'tu_password'
GRANT ALL PRIVILEGES ON DATABASE mercastats TO mercastats_app;
\q

# Ejecutar schema
psql -U postgres -d mercastats -f sql/schema/schema.sql

# 3. Configurar variables de entorno
cp .env.example .env
# Edita .env con tus valores

# 4. Instalar dependencias Python para OCR
cd ocr-service
python -m venv .venv
source .venv/bin/activate   # (en Linux/Mac) o .venv\Scripts\Activate.ps1 (en Windows)
pip install -r requirements.txt
cd ..

# 5. Compilar proyecto Rust
cargo build

# 6. Preparar SQLx
cargo sqlx prepare --workspace

# 7. Ejecutar Backend y Frontend
node dev.js
# o en terminales separadas:
# cd backend && cargo watch -x run
# cd frontend && trunk serve
```

La aplicación frontend estará corriendo en `http://127.0.0.1:3000` y el backend en `http://127.0.0.1:8000`.

-----

## 📂 Estructura del Proyecto

```
mercastats/
├── backend/
│   ├── src/
│   │   ├── main.rs
│   │   ├── config.rs
│   │   ├── db/
│   │   ├── error.rs
│   │   ├── middleware/
│   │   ├── models/
│   │   ├── routes/
│   │   ├── schema/
│   │   └── services/
│   └── Cargo.toml
│
├── frontend/
│   ├── src/
│   │   ├── main.rs
│   │   ├── api/
│   │   ├── components/
│   │   └── pages/
│   ├── index.html
│   ├── Trunk.toml
│   └── Cargo.toml
│
├── ocr-service/
│   ├── src/
│   │   ├── processor.py
│   │   ├── services/
│   │   ├── models.py
│   │   └── constants.py
│   └── requirements.txt
│
├── sql/schema/schema.sql
├── docs/
├── .env.example
├── claude.md
├── Cargo.toml
└── README.md
```

-----

## 🧪 Testing

```powershell
# Ejecutar todos los tests
cargo test --workspace

# Backend con logs
cd backend
cargo test -- --nocapture

# Tests específicos
cargo test db::users -- --nocapture
```

-----

## 📚 Documentación

### Para Desarrolladores
- [claude.md](claude.md)  
- [docs/BACKEND_TICKET_INGESTION_PLAN.md](docs/BACKEND_TICKET_INGESTION_PLAN.md)  
- [docs/OCR_INTEGRATION_NOTES.md](docs/OCR_INTEGRATION_NOTES.md)  
- [sql/schema/schema.sql](sql/schema/schema.sql)  
- [frontend/README.md](frontend/README.md)  
- [ocr-service/README.md](ocr-service/README.md)  

### Recursos Externos
- [Rust Book](https://doc.rust-lang.org/book/)  
- [Axum Documentation](https://docs.rs/axum/)  
- [SQLx Guide](https://github.com/launchbadge/sqlx)  
- [Leptos Book](https://leptos-rs.github.io/leptos/)  
- [PostgreSQL Docs](https://www.postgresql.org/docs/16/)  

-----

## 🛣️ Roadmap

### Implementado ✅
- Setup del Proyecto  
- Base de Datos  
- Backend Core  
- Autenticación  
- OCR y Procesamiento de Tickets  
- Historial y Estadísticas Básicas  
- Frontend Core  
- Páginas Frontend  
- Integración Frontend-Backend  

### Próximos Pasos 📋
- [ ] Estadísticas avanzadas en frontend  
- [ ] Endpoints de estadísticas avanzadas en backend  
- [ ] OCR mejorado (Tesseract)  
- [ ] Gamificación (objetivos, logros)  
- [ ] Refresh tokens  
- [ ] Tests adicionales  
- [ ] Dockerización completa  

-----

## 📄 Licencia

Este proyecto está bajo la licencia MIT. Ver `LICENSE` para más detalles.

-----

## 👨‍💻 Autor

**Juan Carlos**

-----

## 📊 Estado del Proyecto

```
Progreso General: ████████████████░░░░░░ 65%
Backend:          ███████████████████░░░ 75%
Frontend:         ██████████████░░░░░░░ 60%
Workers Python:   ████████░░░░░░░░░░░░░ 40%
Documentación:    ████████████████░░░░░ 80%
Tests:            ████████░░░░░░░░░░░░░ 40%
```

-----

<p align="center">
Hecho con ❤️ y 🦀 (Rust)
</p>
