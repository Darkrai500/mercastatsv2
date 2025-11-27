🏗️ Plan de Migración: Arquitectura de Microservicios (Rust + Python)

Estado: Borrador
Objetivo: Desacoplar el motor de Inteligencia (Python) del Backend (Rust) para permitir escalabilidad horizontal y alta concurrencia.

1. Arquitectura Objetivo

Pasamos de una arquitectura monolítica modular (Python embebido en el proceso de Rust) a una arquitectura de sistemas distribuidos (comunicación HTTP).

1.1. Diagrama de Componentes

graph LR
Client[Frontend / Usuarios] -->|HTTPS| Rust[🦀 Backend (Axum)]

    subgraph "Intelligence Service (Python)"
        LB[Load Balancer / Gunicorn]
        Worker1[🐍 Worker 1]
        Worker2[🐍 Worker 2]
        WorkerN[🐍 Worker N]
    end

    Rust -->|POST /api/ocr| LB
    Rust -->|POST /api/predict| LB

    Worker1 --> OCR[Motor OCR]
    Worker1 --> ML[Motor ML]

2. Estrategia de Concurrencia y Escalabilidad

Para cumplir con tu requisito de "soportar diferentes entradas a la vez", utilizaremos la estrategia estándar de producción para Python (ASGI):

Servidor de Aplicaciones (Uvicorn): Maneja conexiones asíncronas.

Gestor de Procesos (Gunicorn): Python tiene el GIL (Global Interpreter Lock) que impide que un solo proceso use múltiples núcleos de CPU para tareas pesadas (como OCR o ML).

Solución: Lanzaremos el servicio con múltiples workers (procesos independientes).

Fórmula: workers = (2 x Núcleos CPU) + 1.

Si tu servidor tiene 4 núcleos, lanzaremos 9 procesos de Python. Esto permite procesar 9 tickets o predicciones simultáneamente sin bloqueo.

3. Plan de Implementación Paso a Paso

Fase 1: Preparación del Microservicio Python (ocr-service)

El objetivo es convertir la librería actual en una API REST robusta.

1.1. Actualización de Dependencias

Archivo: ocr-service/requirements.txt
Añadir librerías para servidor y ML:

fastapi>=0.100.0
uvicorn[standard]>=0.20.0
gunicorn>=21.0.0
python-multipart>=0.0.6
httpx>=0.24.0

# Futuro ML

pandas
scikit-learn
numpy
joblib

1.2. Reestructuración de la API (src/main.py)

Refactorizar main.py para exponer endpoints claros y tipados.

GET /health: Para que Rust sepa si el servicio está vivo.

POST /ocr/process: Endpoint existente, adaptado para recibir JSON o Multipart.

POST /predict/next-shop: (Nuevo) Placeholder para la predicción.

1.3. Configuración de Workers

Crear un script de entrada (entrypoint.sh o configuración en dev.js) que lance Uvicorn con flags de recarga en desarrollo, pero Gunicorn en producción.

Fase 2: Adaptación del Backend Rust (backend)

Eliminar la dependencia pyo3 y tratar a Python como un servicio externo.

2.1. Limpieza de Dependencias

Eliminar pyo3 de backend/Cargo.toml.

Añadir reqwest = { version = "0.11", features = ["json", "multipart"] }.

2.2. Cliente HTTP Interno

Crear backend/src/services/intelligence_client.rs:

Estructura IntelligenceClient que mantenga un reqwest::Client (pool de conexiones HTTP).

Manejo de Timeouts: Si Python tarda > 30s, cortar y devolver error al usuario.

Manejo de Reintentos: Si Python da error 503 (sobrecarga), reintentar 2 veces antes de fallar.

2.3. Refactorización de Servicios

Modificar backend/src/services/ocr.rs: Sustituir la llamada Python::with_gil por client.post(...).send().await.

Eliminar init_python_worker() (el warm-up ahora es solo comprobar /health).

Fase 3: Configuración del Entorno (DevOps)

3.1. Variables de Entorno

Actualizar .env:

# Antes

# (Nada, era interno)

# Ahora

INTELLIGENCE_SERVICE_URL=[http://127.0.0.1:9000](http://127.0.0.1:9000)
INTELLIGENCE_API_KEY=secret_internal_key # Opcional para seguridad entre servicios

3.2. Orquestación (dev.js)

Actualizar tu script dev.js para que lance el servicio de Python como un proceso independiente en el puerto 9000 antes de lanzar el Backend y Frontend.

4. Contrato de API (Especificación JSON)

Para asegurar la comunicación robusta, definimos estrictamente los mensajes.

4.1. OCR (Input)

// POST /ocr/process
{
"ticket_id": "uuid-v4",
"file_name": "ticket.pdf",
"file_content_b64": "JVBERi0xLj..."
}

4.2. Predicción (Input - Futuro)

Dejaremos la estructura lista para el siguiente sprint.

// POST /predict/next-shop
{
"user_context": {
"current_time": "2025-10-27T09:00:00",
"is_weekend": false
},
"purchase_history_summary": [ ... ]
}

5. Listado de Tareas Inmediatas

[Python] Instalar nuevas dependencias (gunicorn, scikit-learn).

[Python] Reescribir ocr-service/src/main.py para usar FastAPI con rutas dedicadas y Modelos Pydantic estrictos.

[Rust] Añadir reqwest y crear el módulo cliente.

[Rust] Sustituir la lógica de services/ocr.rs.
