"""
Configuracion base para ejecutar el servicio FastAPI con Gunicorn + UvicornWorker.

Usa la formula de produccion recomendada: workers = (2 * CPUs) + 1.
"""

import multiprocessing
import os

host = os.getenv("INTELLIGENCE_HOST", "0.0.0.0")
port = os.getenv("INTELLIGENCE_PORT", "8001")

bind = f"{host}:{port}"
worker_class = "uvicorn.workers.UvicornWorker"
workers = int(os.getenv("INTELLIGENCE_WORKERS", (multiprocessing.cpu_count() * 2) + 1))
timeout = int(os.getenv("INTELLIGENCE_TIMEOUT", "30"))
graceful_timeout = int(os.getenv("INTELLIGENCE_GRACEFUL_TIMEOUT", "30"))
accesslog = "-"
errorlog = "-"
keepalive = 5
