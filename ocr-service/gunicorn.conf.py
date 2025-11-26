"""
Configuracion base para ejecutar el servicio FastAPI con Gunicorn + UvicornWorker.

Usa la formula de produccion recomendada: workers = (2 * CPUs) + 1.
"""

import multiprocessing
import os

host = os.getenv("OCR_HOST", "0.0.0.0")
port = os.getenv("OCR_PORT", "9000")

bind = f"{host}:{port}"
worker_class = "uvicorn.workers.UvicornWorker"
workers = int(os.getenv("OCR_WORKERS", (multiprocessing.cpu_count() * 2) + 1))
timeout = int(os.getenv("OCR_TIMEOUT", "30"))
graceful_timeout = int(os.getenv("OCR_GRACEFUL_TIMEOUT", "30"))
accesslog = "-"
errorlog = "-"
keepalive = 5
