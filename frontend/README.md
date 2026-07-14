# Frontend de Mercastats

SPA escrita en Rust con Leptos 0.6 y compilada a WebAssembly mediante Trunk.

## Requisitos

```bash
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
```

## Desarrollo

Con la API disponible en `http://localhost:8000`:

```bash
cd frontend
trunk serve
```

Trunk sirve la aplicación en <http://localhost:8080>. Para probar el mismo proxy que utiliza el despliegue, levanta el stack completo desde la raíz con `docker compose up --build` y abre <http://localhost:3000>.

## Build de producción

```bash
cd frontend
trunk build --release
```

El resultado se genera en `frontend/dist/` y no se versiona.
