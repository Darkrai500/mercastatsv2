# 📊 Mercastats

> **Análisis estadístico inteligente de tus compras del Mercadona**

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![PostgreSQL](https://img.shields.io/badge/PostgreSQL-16+-blue.svg)](https://www.postgresql.org/)
[![License](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![Status](https://img.shields.io/badge/Status-En%20Desarrollo-yellow.svg)](https://github.com/tu-usuario/mercastats)

---

## 🎯 ¿Qué es Mercastats?

Mercastats es una aplicación web full-stack que te permite:

- 📸 **Subir tickets** de compra (PDF o imágenes)
- 📊 **Visualizar estadísticas** de tus hábitos de consumo
- 💰 **Calcular tu inflación personal** basada en tus productos favoritos
- 📈 **Detectar tendencias** en tus compras
- 🎯 **Establecer objetivos** de ahorro mensuales
- 🏆 **Desbloquear logros** mientras haces tus compras más inteligentes

---

## ✨ Características Principales

### 📊 Estadísticas Básicas (MVP)
- ✅ Gasto medio y desviación estándar
- ✅ Productos más comprados (ranking)
- ✅ Evolución del gasto mensual/semanal
- ✅ Distribución de gasto por categorías
- ✅ Histórico completo desde el inicio

### 🔍 Análisis Avanzados
- 🔎 Detección de tendencias de consumo
- 🔎 Cálculo de inflación personalizada
- 🔎 Comparativa de ticket medio por tienda
- 🔎 Predicción de gasto del próximo mes

### 🎮 Gamificación
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
| **OCR** | Python + Tesseract | Ecosistema maduro de visión por computadora |
| **ML** | Python + scikit-learn | Prototipado rápido de modelos predictivos |

---

## 🚀 Quick Start

### Prerrequisitos

```powershell
# Instalar Rust (https://rustup.rs/)
rustup --version  # 1.75+

# Instalar PostgreSQL (https://www.postgresql.org/download/)
psql --version    # 16+

# Herramientas adicionales
cargo install sqlx-cli --no-default-features --features postgres
cargo install cargo-watch
```

### Instalación

```powershell
# 1. Clonar el repositorio
git clone https://github.com/tu-usuario/mercastats.git
cd mercastats

# 2. Configurar base de datos
psql -U postgres
CREATE DATABASE mercastats;
CREATE USER mercastats_app WITH PASSWORD 'tu_password';
GRANT ALL PRIVILEGES ON DATABASE mercastats TO mercastats_app;
\q

# Ejecutar schema
psql -U postgres -d mercastats -f sql/schema/schema.sql

# 3. Configurar variables de entorno
# Copiar .env.example a .env y editar
DATABASE_URL=postgres://mercastats_app:tu_password@localhost:5432/mercastats
RUST_LOG=debug
JWT_SECRET=genera_un_secreto_seguro_aqui

# 4. Compilar y ejecutar
cargo build
cd backend
cargo run
```

La aplicación estará corriendo en `http://localhost:8000`

---

## 📂 Estructura del Proyecto

```
mercastats/
├── backend/              # Backend en Rust
│   ├── src/
│   │   ├── main.rs      # Punto de entrada
│   │   ├── models/      # Modelos de dominio
│   │   ├── routes/      # Endpoints HTTP
│   │   ├── services/    # Lógica de negocio
│   │   └── db/          # Acceso a datos
│   └── tests/           # Tests de integración
│
├── frontend/            # Frontend Leptos (próximamente)
├── ocr-service/         # Worker Python OCR (futuro)
├── ml-service/          # Worker Python ML (futuro)
├── sql/schema/          # Scripts SQL
└── docs/                # Documentación adicional
```

---

## 🧪 Testing

```powershell
# Ejecutar todos los tests
cargo test

# Tests con logs visibles
cargo test -- --nocapture

# Tests específicos
cargo test test_create_user

# Tests de integración
cargo test --test integration
```

---

## 📚 Documentación

### Para Desarrolladores

- **[claude.md](claude.md)** - Guía completa para Claude Code y desarrolladores
- **[MERCASTATS_TECH_STACK.md](docs/MERCASTATS_TECH_STACK.md)** - Especificación técnica detallada
- **[MERCASTATS_SCHEMA_GUIDE.md](docs/MERCASTATS_SCHEMA_GUIDE.md)** - Guía del schema de base de datos

### Recursos Externos

- [Rust Book](https://doc.rust-lang.org/book/)
- [Axum Documentation](https://docs.rs/axum/)
- [SQLx Guide](https://github.com/launchbadge/sqlx)
- [PostgreSQL Docs](https://www.postgresql.org/docs/16/)

---

## 🛣️ Roadmap

### ✅ Fase 1: MVP Backend (En Progreso)
- [x] Setup del proyecto
- [x] Schema de base de datos completo
- [ ] Sistema de logging
- [ ] CRUD de usuarios
- [ ] CRUD de compras
- [ ] Endpoints de estadísticas básicas

### 📋 Fase 2: Autenticación (Próximo)
- [ ] Sistema JWT
- [ ] Registro de usuarios
- [ ] Login seguro
- [ ] Middleware de autenticación

### 📊 Fase 3: Estadísticas Avanzadas
- [ ] Análisis de tendencias
- [ ] Cálculo de inflación
- [ ] Predicciones de gasto
- [ ] Comparativas temporales

### 🎨 Fase 4: Frontend
- [ ] Setup de Leptos
- [ ] Dashboard principal
- [ ] Gráficos interactivos
- [ ] Upload de tickets

---

## 📄 Licencia

Este proyecto está bajo la licencia MIT. Ver `LICENSE` para más detalles.

---

## 👨‍💻 Autor

**Juan Carlos**

---

## 📊 Estado del Proyecto

```
Progreso General: ███████░░░░░░░░░░░░░░ 35%

Backend:          ████████░░░░░░░░░░░░░ 40%
Frontend:         ░░░░░░░░░░░░░░░░░░░░░  0%
Workers Python:   ░░░░░░░░░░░░░░░░░░░░░  0%
Documentación:    ████████████████░░░░░ 80%
Tests:            ██████░░░░░░░░░░░░░░░ 30%
```

---

<p align="center">
  Hecho con ❤️ y 🦀 (Rust)
</p>
