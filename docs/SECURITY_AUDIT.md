# Estado de seguridad de Mercastats

Última revisión local: 2026-07-14.

Este documento describe controles verificables en el código y trabajo pendiente. No sustituye una auditoría profesional del despliegue.

## Controles presentes

- Consultas SQL parametrizadas mediante SQLx.
- Contraseñas protegidas con bcrypt (coste 12).
- JWT firmado y con expiración.
- Identidad derivada del bearer token en el backend, no de parámetros del cliente.
- Tamaño y tipo de ticket validados antes del procesamiento.
- CORS restringido a la lista `CORS_ORIGINS` en Rust y FastAPI.
- Clave compartida `INTELLIGENCE_API_KEY` comprobada por el servicio Python cuando está configurada.
- Timeouts y reintentos acotados en las llamadas Rust → Python.
- Logs de OCR y predicción sin email, factura, cesta, total ni identificador de usuario.

## Configuración obligatoria de producción

1. Generar valores independientes y aleatorios para `JWT_SECRET`, `POSTGRES_PASSWORD` e `INTELLIGENCE_API_KEY`.
2. Definir `CORS_ORIGINS` solo con los orígenes HTTPS del despliegue.
3. No exponer PostgreSQL ni el servicio de inteligencia directamente a Internet; usar una configuración Compose/infra específica de producción.
4. Mantener `.env` fuera de Git y de imágenes o artefactos publicados.

## Trabajo pendiente

- Rate limiting en autenticación y endpoints de procesamiento.
- Cabeceras HTTP de endurecimiento en el proxy (`Content-Security-Policy`, `X-Content-Type-Options`, `Referrer-Policy` y HSTS bajo HTTPS).
- Sustituir el uso localizado de `js_sys::eval` en `frontend/src/components/chart.rs`.
- Pruebas de integración que verifiquen CORS, autenticación interna y límites de subida.
- Política operativa documentada de retención y borrado de tickets.

## Privacidad

Los tickets pueden revelar dirección, fecha, hábitos de compra y datos parciales de pago. Las fixtures, capturas y ejemplos versionados deben ser sintéticos. Los logs de producción no deben contener texto OCR, nombres de producto, importes, facturas, emails ni identificadores estables de usuario.
