# Mercastats Frontend

Frontend de la aplicación Mercastats construido con **Leptos** (Rust + WebAssembly) y **Tailwind CSS**.

## 🎨 Diseño

El frontend sigue una filosofía de diseño **minimalista y moderno** inspirada en:

- **Apple** - Espacios en blanco, tipografía limpia
- **Linear** - Interfaz focalizada, animaciones sutiles
- **Stripe** - Profesional, datos como protagonistas

### Características visuales

- ✨ Animaciones sutiles (fade-in, slide-up)
- 🎨 Paleta de colores moderna (azul primary, morado accent)
- 📱 Diseño responsive (mobile-first)
- 🌓 Preparado para dark mode (futuro)
- ♿ Accesibilidad (ARIA labels, contraste)

## 📁 Estructura del Proyecto

```
frontend/
├── src/
│   ├── main.rs              # Punto de entrada, configuración del router
│   ├── components/          # Componentes reutilizables
│   │   ├── mod.rs
│   │   ├── button.rs        # Botón con variantes (primary, outline, ghost)
│   │   ├── input.rs         # Input con validación y errores
│   │   └── card.rs          # Contenedor con sombra y padding
│   ├── pages/               # Páginas/vistas
│   │   ├── mod.rs
│   │   ├── login.rs         # Página de inicio de sesión
│   │   └── upload.rs        # Página de subida de tickets
│   └── api/                 # Cliente API para backend
│       ├── mod.rs           # Configuración base (URL, tokens)
│       ├── auth.rs          # Endpoints de autenticación
│       └── tickets.rs       # Endpoints de tickets
├── index.html               # HTML base con Tailwind CDN
├── Trunk.toml               # Configuración de Trunk (build tool)
├── Cargo.toml               # Dependencias del frontend
└── README.md                # Este archivo
```

## 🚀 Instalación y Ejecución

### Prerrequisitos

```powershell
# Instalar Trunk (build tool para Leptos)
cargo install trunk

# Instalar wasm-bindgen-cli
cargo install wasm-bindgen-cli

# Agregar target wasm32
rustup target add wasm32-unknown-unknown
```

### Desarrollo

```powershell
# Navegar al directorio del frontend
cd frontend

# Ejecutar en modo desarrollo (con hot-reload)
trunk serve

# La aplicación estará disponible en:
# http://127.0.0.1:3000
```

El comando `trunk serve` hace:
- Compila el código Rust a WebAssembly
- Inicia un servidor de desarrollo con hot-reload
- Proxy para requests a la API del backend (localhost:8000)

### Build para Producción

```powershell
# Build optimizado
trunk build --release

# Los archivos se generan en frontend/dist/
# - index.html
# - *.wasm (WebAssembly binary)
# - *.js (JavaScript glue code)
```

## 🎯 Páginas Implementadas

### 1. Login (`/`)

Página de inicio de sesión con:
- Formulario de email/password con validación
- Mensaje de error/éxito
- Checkbox "Recordarme"
- Botón de "Olvidé mi contraseña"
- Botón de login con Google (placeholder)
- Link de registro

**Características:**
- Validación en cliente (email válido, campos requeridos)
- Almacena token JWT en localStorage
- Redirección automática a `/upload` tras login exitoso

### 2. Upload (`/upload`)

Página principal de la aplicación con:
- Header con logo y botón de logout
- Área de drag & drop para subir tickets
- Preview de imágenes
- Información del archivo seleccionado
- Botones de subir/cancelar
- Cards con estadísticas (tickets, gasto, productos)
- Sección de consejos y privacidad

**Características:**
- Protección de ruta (redirección a `/` si no hay sesión)
- Drag & drop de archivos
- Soporte para PDF e imágenes
- Preview en tiempo real para imágenes
- Validación de tamaño (max 10MB)
- Feedback visual (loading, success, error)

### 3. Not Found (`/*any`)

Página de error 404 con diseño minimalista y botón para volver al inicio.

## 🧩 Componentes Reutilizables

### Button

Componente de botón con múltiples variantes:

```rust
use crate::components::{Button, ButtonVariant};

<Button
    variant=ButtonVariant::Primary  // Primary, Secondary, Outline, Ghost
    full_width=true
    loading=false
    disabled=false
    on_click=Some(Box::new(|| { /* handler */ }))
>
    "Texto del botón"
</Button>
```

### Input

Componente de input con label, validación y errores:

```rust
use crate::components::Input;

<Input
    label=Some("Email".to_string())
    placeholder="tu@email.com".to_string()
    input_type="email".to_string()
    value=create_rw_signal(String::new())
    error=Some("Email inválido".to_string())
    required=true
    name=Some("email".to_string())
/>
```

### Card

Contenedor con sombra y bordes redondeados:

```rust
use crate::components::Card;

<Card title=Some("Título".to_string()) padding=true>
    <p>"Contenido de la tarjeta"</p>
</Card>
```

## 🔌 Cliente API

### Autenticación

```rust
use crate::api::auth::{login_user, LoginRequest};

let request = LoginRequest {
    email: "usuario@email.com".to_string(),
    password: "password123".to_string(),
};

match login_user(request).await {
    Ok(response) => {
        // response.token - JWT token
        // response.user.email - Email del usuario
    }
    Err(error) => {
        // Manejo de error
    }
}
```

### Tickets

```rust
use crate::api::tickets::upload_ticket;
use web_sys::File;

match upload_ticket(file).await {
    Ok(response) => {
        // response.ticket_id - ID del ticket subido
        // response.message - Mensaje de confirmación
    }
    Err(error) => {
        // Manejo de error
    }
}
```

## 🎨 Customización de Estilos

El proyecto usa **Tailwind CSS** vía CDN (modo desarrollo). Para producción, se recomienda:

1. Instalar Tailwind localmente:

```powershell
npm init -y
npm install -D tailwindcss postcss autoprefixer
npx tailwindcss init
```

2. Configurar `tailwind.config.js`:

```javascript
module.exports = {
  content: ["./src/**/*.rs", "./index.html"],
  theme: {
    extend: {
      colors: {
        primary: { /* colores personalizados */ },
        accent: { /* colores personalizados */ },
      },
    },
  },
};
```

3. Crear `style/input.css`:

```css
@tailwind base;
@tailwind components;
@tailwind utilities;
```

4. Actualizar `index.html` para usar el CSS generado.

## 🐛 Debugging

### Logs en consola

```rust
use log::info;

info!("Mensaje de debug");
```

Los logs aparecen en la consola del navegador (F12).

### Inspeccionar WebAssembly

1. Abrir DevTools (F12)
2. Ir a la pestaña "Sources"
3. Buscar archivos `.wasm`
4. Usar breakpoints en código Rust

## 🔒 Autenticación y Seguridad

- Los tokens JWT se almacenan en **localStorage**
- El token se envía en el header `Authorization: Bearer <token>`
- Las rutas protegidas redirigen a `/` si no hay token
- Errores 401 del backend limpian el localStorage

### Mejoras futuras:

- Usar **httpOnly cookies** en lugar de localStorage
- Implementar **refresh tokens**
- Agregar **CSRF protection**
- Implementar **rate limiting** en el cliente

## 📦 Dependencias Principales

| Crate            | Versión | Descripción                            |
| ---------------- | ------- | -------------------------------------- |
| leptos           | 0.6     | Framework reactivo                     |
| leptos_router    | 0.6     | Enrutamiento SPA                       |
| leptos_meta      | 0.6     | Meta tags (SEO)                        |
| gloo-net         | 0.5     | HTTP client para WebAssembly           |
| serde            | 1.0     | Serialización JSON                     |
| wasm-bindgen     | 0.2     | Bindings JavaScript ↔ Rust             |
| web-sys          | 0.3     | APIs del navegador (DOM, localStorage) |
| console_log      | 1.0     | Logging en consola del navegador       |

## 🚧 Roadmap

### Implementado ✅

- [x] Setup del proyecto con Leptos
- [x] Componentes base (Button, Input, Card)
- [x] Página de Login
- [x] Página de Upload de tickets
- [x] Cliente API (auth, tickets)
- [x] Enrutamiento básico
- [x] Manejo de errores
- [x] Diseño responsive

### Pendiente 📋

- [ ] Dashboard con estadísticas
- [ ] Gráficos interactivos (Chart.js o Plotters)
- [ ] Lista de tickets históricos
- [ ] Detalle de ticket individual
- [ ] Página de perfil de usuario
- [ ] Dark mode
- [ ] Tests unitarios e integración
- [ ] Internacionalización (i18n)
- [ ] PWA (Progressive Web App)
- [ ] Optimización de bundle size

## 📞 Soporte

Para problemas o preguntas:

1. Revisa la [documentación de Leptos](https://leptos-rs.github.io/leptos/)
2. Revisa la [documentación de Trunk](https://trunkrs.dev/)
3. Consulta el archivo `CLAUDE.md` en la raíz del proyecto

---

**Última actualización**: 24 de octubre de 2025
**Versión**: 0.1.0
**Autor**: Juan Carlos
