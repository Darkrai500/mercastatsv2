# 📊 Guía del Dashboard de Mercastats

## 📋 Contenido

1. [Estructura del Dashboard](#estructura-del-dashboard)
2. [Cómo añadir una nueva opción al menú](#cómo-añadir-una-nueva-opción-al-menú)
3. [Estructura de archivos](#estructura-de-archivos)
4. [Ejemplo completo](#ejemplo-completo)

---

## 🏗️ Estructura del Dashboard

El Dashboard de Mercastats está construido con un diseño de **barra lateral fija** (sidebar) y un **área de contenido principal** que cambia dinámicamente según la opción seleccionada en el menú.

### Componentes principales:

```
Dashboard (dashboard.rs)
├── Sidebar (sidebar.rs) - Menú lateral con navegación
└── Contenido dinámico - Cambia según la vista seleccionada
    ├── Upload (upload.rs) - Página para subir tickets
    └── ExamplePage (example.rs) - Página de ejemplo
```

### Estado de navegación:

El estado de navegación se maneja mediante un enum `DashboardView` que define todas las vistas disponibles:

```rust
pub enum DashboardView {
    Upload,
    Example,
}
```

---

## 🚀 Cómo añadir una nueva opción al menú

Para añadir una nueva opción al menú del Dashboard, sigue estos pasos:

### **Paso 1: Añadir la nueva vista al enum `DashboardView`**

Edita el archivo `frontend/src/components/sidebar.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DashboardView {
    Upload,
    Example,
    MiNuevaVista,  // ← Añadir aquí
}
```

### **Paso 2: Crear la nueva página/componente**

Crea un nuevo archivo en `frontend/src/pages/`, por ejemplo `mi_nueva_vista.rs`:

```rust
use leptos::*;

/// Mi nueva vista personalizada
#[component]
pub fn MiNuevaVista() -> impl IntoView {
    view! {
        <div class="space-y-6">
            // Header
            <div>
                <h1 class="text-3xl font-bold text-gray-900 mb-2">
                    "Mi Nueva Vista"
                </h1>
                <p class="text-gray-600">
                    "Descripción de tu nueva funcionalidad"
                </p>
            </div>

            // Contenido principal
            <div class="bg-white border border-gray-200 rounded-lg p-6">
                <p>"Aquí va el contenido de tu nueva vista"</p>
            </div>
        </div>
    }
}
```

### **Paso 3: Registrar la nueva página en el módulo pages**

Edita `frontend/src/pages/mod.rs`:

```rust
pub mod login;
pub mod register;
pub mod upload;
pub mod dashboard;
pub mod example;
pub mod mi_nueva_vista;  // ← Añadir el módulo

pub use login::Login;
pub use register::Register;
pub use upload::Upload;
pub use dashboard::Dashboard;
pub use example::ExamplePage;
pub use mi_nueva_vista::MiNuevaVista;  // ← Exportar el componente
```

### **Paso 4: Añadir la opción al menú del Sidebar**

Edita `frontend/src/components/sidebar.rs`, en la sección de navegación añade un nuevo botón:

```rust
// Opción: Mi Nueva Vista
<button
    class=move || {
        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
        if current_view.get() == DashboardView::MiNuevaVista {
            format!("{} bg-primary-50 text-primary-700", base)
        } else {
            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
        }
    }
    on:click=move |_| on_view_change.call(DashboardView::MiNuevaVista)
>
    // Icono (puedes cambiarlo por el que prefieras)
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z"></path>
    </svg>
    <span>"Mi Nueva Vista"</span>
    {move || if current_view.get() == DashboardView::MiNuevaVista {
        view! {
            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
        }.into_view()
    } else {
        view! {}.into_view()
    }}
</button>
```

**Ubicación recomendada:** Justo después del botón "Estadísticas" (ExamplePage) en el archivo `sidebar.rs`.

### **Paso 5: Añadir el caso en el match del Dashboard**

Edita `frontend/src/pages/dashboard.rs`:

```rust
use crate::pages::{Upload, ExamplePage, MiNuevaVista};  // ← Importar

// En el view!
{move || match current_view.get() {
    DashboardView::Upload => view! { <Upload /> }.into_view(),
    DashboardView::Example => view! { <ExamplePage /> }.into_view(),
    DashboardView::MiNuevaVista => view! { <MiNuevaVista /> }.into_view(),  // ← Añadir
}}
```

### **Paso 6: Compilar y probar**

```powershell
cd frontend
cargo check  # Verificar que no hay errores
trunk serve  # Iniciar el servidor de desarrollo
```

---

## 📁 Estructura de archivos

```
frontend/
├── src/
│   ├── components/
│   │   ├── sidebar.rs         ← Define DashboardView y el menú lateral
│   │   └── mod.rs
│   ├── pages/
│   │   ├── dashboard.rs       ← Componente principal del Dashboard
│   │   ├── upload.rs          ← Vista de upload (subpágina)
│   │   ├── example.rs         ← Vista de ejemplo (subpágina)
│   │   ├── mi_nueva_vista.rs  ← Tu nueva vista (subpágina)
│   │   └── mod.rs             ← Exporta todas las páginas
│   └── lib.rs                 ← Define las rutas principales
└── DASHBOARD_GUIDE.md         ← Este archivo
```

---

## 💡 Ejemplo completo

Aquí hay un ejemplo completo de cómo añadir una vista de "Historial de Compras":

### 1. Enum en `sidebar.rs`:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum DashboardView {
    Upload,
    Example,
    History,  // ← Nueva
}
```

### 2. Nuevo archivo `history.rs`:

```rust
use leptos::*;

#[component]
pub fn History() -> impl IntoView {
    view! {
        <div class="space-y-6">
            <div>
                <h1 class="text-3xl font-bold text-gray-900 mb-2">
                    "Historial de Compras"
                </h1>
                <p class="text-gray-600">
                    "Consulta todos tus tickets anteriores"
                </p>
            </div>

            <div class="bg-white border border-gray-200 rounded-lg p-6">
                <p class="text-gray-600">
                    "Aquí se mostrarán todos tus tickets..."
                </p>
            </div>
        </div>
    }
}
```

### 3. En `pages/mod.rs`:

```rust
pub mod history;
pub use history::History;
```

### 4. En `sidebar.rs` (dentro de `<nav>`):

```rust
// Opción: Historial
<button
    class=move || {
        let base = "w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm font-medium transition-all";
        if current_view.get() == DashboardView::History {
            format!("{} bg-primary-50 text-primary-700", base)
        } else {
            format!("{} text-gray-700 hover:bg-gray-100 hover:text-gray-900", base)
        }
    }
    on:click=move |_| on_view_change.call(DashboardView::History)
>
    <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"></path>
    </svg>
    <span>"Historial"</span>
    {move || if current_view.get() == DashboardView::History {
        view! {
            <div class="ml-auto w-1.5 h-1.5 bg-primary-600 rounded-full"></div>
        }.into_view()
    } else {
        view! {}.into_view()
    }}
</button>
```

### 5. En `dashboard.rs`:

```rust
use crate::pages::{Upload, ExamplePage, History};

{move || match current_view.get() {
    DashboardView::Upload => view! { <Upload /> }.into_view(),
    DashboardView::Example => view! { <ExamplePage /> }.into_view(),
    DashboardView::History => view! { <History /> }.into_view(),
}}
```

---

## 🎨 Iconos recomendados

Aquí tienes algunos iconos SVG de Heroicons que puedes usar:

```html
<!-- Upload -->
<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
  <path
    stroke-linecap="round"
    stroke-linejoin="round"
    stroke-width="2"
    d="M7 16a4 4 0 01-.88-7.903A5 5 0 1115.9 6L16 6a5 5 0 011 9.9M15 13l-3-3m0 0l-3 3m3-3v12"
  ></path>
</svg>

<!-- Estadísticas -->
<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
  <path
    stroke-linecap="round"
    stroke-linejoin="round"
    stroke-width="2"
    d="M9 19v-6a2 2 0 00-2-2H5a2 2 0 00-2 2v6a2 2 0 002 2h2a2 2 0 002-2zm0 0V9a2 2 0 012-2h2a2 2 0 012 2v10m-6 0a2 2 0 002 2h2a2 2 0 002-2m0 0V5a2 2 0 012-2h2a2 2 0 012 2v14a2 2 0 01-2 2h-2a2 2 0 01-2-2z"
  ></path>
</svg>

<!-- Historial -->
<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
  <path
    stroke-linecap="round"
    stroke-linejoin="round"
    stroke-width="2"
    d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z"
  ></path>
</svg>

<!-- Configuración -->
<svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
  <path
    stroke-linecap="round"
    stroke-linejoin="round"
    stroke-width="2"
    d="M10.325 4.317c.426-1.756 2.924-1.756 3.35 0a1.724 1.724 0 002.573 1.066c1.543-.94 3.31.826 2.37 2.37a1.724 1.724 0 001.065 2.572c1.756.426 1.756 2.924 0 3.35a1.724 1.724 0 00-1.066 2.573c.94 1.543-.826 3.31-2.37 2.37a1.724 1.724 0 00-2.572 1.065c-.426 1.756-2.924 1.756-3.35 0a1.724 1.724 0 00-2.573-1.066c-1.543.94-3.31-.826-2.37-2.37a1.724 1.724 0 00-1.065-2.572c-1.756-.426-1.756-2.924 0-3.35a1.724 1.724 0 001.066-2.573c-.94-1.543.826-3.31 2.37-2.37.996.608 2.296.07 2.572-1.065z"
  ></path>
  <path
    stroke-linecap="round"
    stroke-linejoin="round"
    stroke-width="2"
    d="M15 12a3 3 0 11-6 0 3 3 0 016 0z"
  ></path>
</svg>
```

Más iconos disponibles en: [Heroicons](https://heroicons.com/)

---

## ✅ Checklist para añadir una nueva vista

- [ ] Añadir variante al enum `DashboardView` en `sidebar.rs`
- [ ] Crear nuevo archivo de componente en `pages/`
- [ ] Registrar módulo en `pages/mod.rs`
- [ ] Añadir botón en el sidebar (`sidebar.rs`)
- [ ] Añadir caso en el match de `dashboard.rs`
- [ ] Ejecutar `cargo check` para verificar errores
- [ ] Probar la navegación con `trunk serve`

---

## 🐛 Troubleshooting

### El proyecto no compila después de añadir una vista

- Verifica que hayas importado el componente en `dashboard.rs`
- Asegúrate de que el nombre del archivo coincide con el módulo en `mod.rs`
- Ejecuta `cargo clean` y vuelve a compilar

### La opción del menú no aparece

- Verifica que hayas añadido el botón dentro del elemento `<nav>` en `sidebar.rs`
- Revisa que el `on:click` llame a `on_view_change.call(DashboardView::TuVista)`

### El componente no se renderiza

- Verifica que hayas añadido el caso correspondiente en el `match` de `dashboard.rs`
- Asegúrate de que el enum `DashboardView` esté sincronizado entre `sidebar.rs` y `dashboard.rs`

# 🎨 Guía de Animaciones del Dashboard

## 📋 Contenido

1. [Animaciones implementadas](#animaciones-implementadas)
2. [Cómo funcionan](#cómo-funcionan)
3. [Personalizando animaciones](#personalizando-animaciones)
4. [Mejores prácticas](#mejores-prácticas)

---

## 🎬 Animaciones implementadas

El Dashboard de Mercastats cuenta con animaciones suaves y elegantes que siguen el diseño minimalista del proyecto.

### Transición de subpáginas

Cuando cambias entre vistas en el Dashboard (ej: de "Subir ticket" a "Estadísticas"), la transición es suave:

- **Duración:** 0.5 segundos
- **Efecto:** Fade In (opacidad: 0 → 1)
- **Easing:** ease-in-out
- **Sensación:** Cambio suave y elegante

```
[Página A: Visible] → [Transición 0.5s] → [Página B: Visible]
```

### Otras animaciones disponibles

| Nombre               | Duración | Efecto                   | Caso de uso               |
| -------------------- | -------- | ------------------------ | ------------------------- |
| `animate-fade-in`    | 0.5s     | Opacidad (0 → 1)         | Transiciones de contenido |
| `animate-slide-up`   | 0.4s     | Sube desde abajo + fade  | Entrada de elementos      |
| `animate-slide-down` | 0.4s     | Baja desde arriba + fade | Desplegables              |
| `animate-scale-in`   | 0.3s     | Escala (0.95 → 1) + fade | Popups, modales           |

---

## 🔧 Cómo funcionan

### Sistema actual de transiciones

El cambio entre subpáginas usa tres mecanismos:

1. **Key único por vista** - Leptos detecta cambios en la `key` y remonta el componente
2. **Clase CSS `animate-fade-in`** - Aplicada automáticamente al contenedor
3. **Transición CSS** - `transition-all duration-300` añade suavidad

```rust
{move || {
    let current = current_view.get();
    view! {
        <div
            class="transition-all duration-300 ease-in-out animate-fade-in"
            key=format!("{:?}", current)  // ← Key única por vista
        >
            {match current {
                DashboardView::Upload => view! { <Upload /> }.into_view(),
                DashboardView::Example => view! { <ExamplePage /> }.into_view(),
            }}
        </div>
    }
}}
```

### Configuración en Tailwind

Todas las animaciones están definidas en `frontend/index.html`:

```javascript
animation: {
  'fade-in': 'fadeIn 0.5s ease-in-out',
  'slide-up': 'slideUp 0.4s ease-out',
  'slide-down': 'slideDown 0.4s ease-out',
  'scale-in': 'scaleIn 0.3s ease-out',
},
keyframes: {
  fadeIn: {
    '0%': { opacity: '0' },
    '100%': { opacity: '1' },
  },
  // ... más keyframes
}
```

---

## 🎨 Personalizando animaciones

### Cambiar la duración de la transición

En `frontend/src/pages/dashboard.rs`:

```rust
// Más rápido (300ms)
<div class="transition-all duration-300 ease-in-out animate-fade-in">

// Más lento (700ms)
<div class="transition-all duration-700 ease-in-out animate-fade-in">
```

**Opciones:** `duration-75`, `duration-100`, `duration-150`, `duration-200`, `duration-300`, `duration-500`, `duration-700`, `duration-1000`

### Cambiar el tipo de animación

Reemplaza `animate-fade-in` en el Dashboard:

```rust
// Efecto slide up
<div class="transition-all duration-300 ease-in-out animate-slide-up">

// Efecto scale
<div class="transition-all duration-300 ease-in-out animate-scale-in">
```

### Añadir nueva animación personalizada

1. Edita `frontend/index.html` y añade a `keyframes`:

```javascript
keyframes: {
  // ... existentes
  slideLeft: {
    '0%': { transform: 'translateX(20px)', opacity: '0' },
    '100%': { transform: 'translateX(0)', opacity: '1' },
  }
}
```

2. Añade a `animation`:

```javascript
animation: {
  // ... existentes
  'slide-left': 'slideLeft 0.4s ease-out',
}
```

3. Úsala en el Dashboard:

```rust
<div class="transition-all duration-300 ease-in-out animate-slide-left">
```

---

## 📊 Mejores prácticas

### ✅ Hacer

- **Mantener animaciones breves** (300-500ms) para no ralentizar la experiencia
- **Usar easing functions apropiadas**:
  - `ease-in-out`: Cambios suaves (defecto)
  - `ease-out`: Entrada rápida (recomendado para elementos que entran)
  - `ease-in`: Salida gradual
- **Animar opacidad y transforms** (más eficientes)
- **Consistencia:** Usar las mismas animaciones en todo el app

### ❌ No hacer

- **Animaciones muy largas** (>1s) que ralentizan la navegación
- **Animaciones complejas** que requieren mucho cálculo
- **Animar propiedades "heavy"** como `width`, `height`, `top`, `left`
- **Demasiadas animaciones simultáneas** que compitan visualmente

### 🎯 Para subpáginas

El patrón actual es óptimo:

```rust
<div
    class="transition-all duration-300 ease-in-out animate-fade-in"
    key=format!("{:?}", current_view)
>
    {/* contenido */}
</div>
```

**Ventajas:**

- Rápido (300ms es imperceptible)
- Elegante (fade es minimalista)
- Suave (ease-in-out es natural)
- Performante (solo anima opacidad)

---

## 💡 Ejemplos

### Cambiar a animación más lenta

```rust
// Antes (300ms)
<div class="transition-all duration-300 ease-in-out animate-fade-in">

// Después (700ms, más teatral)
<div class="transition-all duration-700 ease-out animate-slide-up">
```

### Combinar múltiples efectos

Aunque no es recomendable para performance, podrías combinar:

```rust
// Opacidad + escala + movimiento
<div class="transition-all duration-500 ease-out animate-scale-in">
```

### Para futuros modales/popups

```rust
// Entrada rápida con escala
<div class="transition-all duration-200 ease-out animate-scale-in">
    {/* Tu modal aquí */}
</div>
```

---

## 🐛 Troubleshooting

### Las animaciones no se ven

**Problema:** El contenido aparece/desaparece sin transición

**Soluciones:**

1. Verifica que `animate-fade-in` esté en la clase
2. Asegúrate de que la `key` cambia cuando cambias de vista
3. Revisa que `transition-all duration-300` esté presente

### Las animaciones son muy lentas

**Reducir duración en Dashboard:**

```rust
class="transition-all duration-200 ease-in-out animate-fade-in"
// O cambiar la animación misma en index.html
```

### Las animaciones causan parpadeo

**Causa:** Hay conflicto entre transiciones y animaciones

**Solución:** Usa solo `animate-fade-in` sin `transition-all`:

```rust
class="animate-fade-in"  // Sin transition-all
```

---

## 📚 Recursos

- [Tailwind CSS Animations](https://tailwindcss.com/docs/animation)
- [MDN: CSS Animations](https://developer.mozilla.org/en-US/docs/Web/CSS/animation)
- [Easing Functions](https://easings.net/)
- [Principles of Motion Design](https://material.io/design/motion/understanding-motion.html)

---

**Última actualización:** 25 de octubre de 2025
**Versión:** 1.0

---

**Fecha de creación:** 25 de octubre de 2025
**Versión:** 1.0
**Autor:** Juan Carlos (con ayuda de Claude Code)
