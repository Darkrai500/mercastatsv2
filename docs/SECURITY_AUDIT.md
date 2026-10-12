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
- Logs de OCR y predicción sin email, factura, cesta, total ni identificador de usuario (el backend registra solo recuentos).
- Las respuestas 422 del servicio de inteligencia no repiten los valores recibidos, para que no acaben en los logs de quien llama.

## Integración con Google y Gmail (añadido 2026-10-12)

Controles implementados y cubiertos por pruebas automáticas (`backend/src/google_flow_tests.rs`, con un Google/Gmail simulado):

- Flujo OAuth de código con PKCE (S256). `state` y verificador viajan en una cookie `HttpOnly; SameSite=Lax` firmada y con caducidad de 10 minutos, que ata el callback al navegador que lo inició; el `state` se compara en tiempo constante.
- El token de acceso permanente se guarda cifrado con AES-256-GCM (nonce aleatorio por cifrado; el email del usuario va como dato asociado, así que un token copiado a otra fila no se descifra). La clave (`TOKEN_ENCRYPTION_KEY`) solo existe en el entorno.
- Ámbito de solo lectura (`gmail.readonly`), sin escritura ni envío. Se comprueba que el usuario no haya desmarcado el permiso de Gmail.
- Un inicio de sesión con Google nunca se enlaza automáticamente con una cuenta de contraseña existente (evita el *pre-hijacking*: el registro con contraseña no verifica el correo). Esas cuentas se vinculan desde dentro de una sesión autenticada.
- Se exige `email_verified` de Google, y una cuenta de Google no puede vincularse a dos usuarios.
- De cada correo solo se importan los adjuntos PDF cuyo remitente **real** coincide con el configurado (la búsqueda `from:` de Gmail también casa con el nombre visible, que es falsificable); se valida la firma `%PDF-`, el tamaño y la profundidad MIME.
- Los errores vuelven al frontend como códigos cortos, nunca como texto de Google; los tokens y perfiles no tienen `Debug` que los imprima.
- Una sola sincronización a la vez por usuario; los fallos transitorios (OCR, red) no marcan el correo como visto; los de contenido se reintentan 3 veces como máximo.
- La cuenta demo no puede conectar Gmail. Desconectar revoca el token en Google y lo borra; borrar el usuario borra en cascada su token y su historial de correos procesados.
- Los logs de la sincronización solo contienen recuentos y códigos de error.

Pendiente específico:

- `gmail.readonly` es un ámbito restringido: abrir la aplicación a usuarios generales exige verificación de Google y evaluación de seguridad externa.
- El JWT de sesión viaja una vez en el fragmento de la URL de retorno (no se envía a servidores; el frontend lo borra del historial al instante). Alternativa más estricta: un código de un solo uso canjeado por POST.
- Rotación de `TOKEN_ENCRYPTION_KEY` (hoy, cambiarla obliga a reconectar).
- Los PDF importados desde Gmail siguen la misma política de retención que los subidos a mano (aún sin definir, ver más abajo).

## Configuración obligatoria de producción

1. Generar valores independientes y aleatorios para `JWT_SECRET`, `POSTGRES_PASSWORD` e `INTELLIGENCE_API_KEY`.
2. Definir `CORS_ORIGINS` solo con los orígenes HTTPS del despliegue.
3. No exponer PostgreSQL ni el servicio de inteligencia directamente a Internet; usar una configuración Compose/infra específica de producción.
4. Mantener `.env` fuera de Git y de imágenes o artefactos publicados.
5. Si se activa Google: `GOOGLE_REDIRECT_URI` con HTTPS (la cookie de estado pasa a llevar `Secure`), un `TOKEN_ENCRYPTION_KEY` propio y guardado fuera de la base de datos (sin él los tokens cifrados son irrecuperables), y la URI registrada en Google Cloud idéntica a la configurada.

## Trabajo pendiente

- Rate limiting en autenticación y endpoints de procesamiento.
- Cabeceras HTTP de endurecimiento en el proxy (`Content-Security-Policy`, `X-Content-Type-Options`, `Referrer-Policy` y HSTS bajo HTTPS).
- Sustituir el uso localizado de `js_sys::eval` en `frontend/src/components/chart.rs`.
- Pruebas de integración que verifiquen CORS, autenticación interna y límites de subida.
- Política operativa documentada de retención y borrado de tickets.

## Privacidad

Con la sincronización de Gmail la aplicación tiene permiso de lectura sobre toda la bandeja del usuario (Google no ofrece un ámbito más estrecho); el código solo lista y descarga los mensajes de un remitente, pero el permiso concedido es más amplio que el uso. Los tickets pueden revelar dirección, fecha, hábitos de compra y datos parciales de pago. Las fixtures, capturas y ejemplos versionados deben ser sintéticos. Los logs de producción no deben contener texto OCR, nombres de producto, importes, facturas, emails ni identificadores estables de usuario.
