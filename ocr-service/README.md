# OCR Service - Mercastats

Worker Python para procesamiento de tickets PDF de Mercadona.

## 🎯 Descripción

Este servicio es responsable de:

- ✅ Extraer texto de PDFs de tickets de Mercadona
- ✅ Identificar información estructurada mediante regex:
  - Número de factura (formato: XXXX-XXX-XXXXXX)
  - Fecha del ticket (formato: DD/MM/YYYY)
  - Total del ticket (en euros)
- ✅ Retornar datos estructurados al backend Rust

## 🏗️ Arquitectura

```
Backend Rust (Axum)
       ↓
   HTTP POST /ocr/process
       ↓
OCR Service (FastAPI)
       ↓
   pdfplumber → Extracción de texto
       ↓
   Regex patterns → Parsing estructurado
       ↓
   JSON response
```

## 📦 Dependencias

- **FastAPI**: Framework web moderno y rápido
- **pdfplumber**: Extracción de texto de PDFs nativos
- **Pydantic**: Validación de datos
- **Loguru**: Logging mejorado
- **Uvicorn**: Servidor ASGI

## 🚀 Setup Local

### 1. Crear entorno virtual

**Windows (PowerShell):**
```powershell
cd ocr-service
python -m venv .venv
.venv\Scripts\Activate.ps1
```

**Linux/macOS:**
```bash
cd ocr-service
python3 -m venv .venv
source .venv/bin/activate
```

### 2. Instalar dependencias

```bash
pip install --upgrade pip
pip install -r requirements.txt
```

### 3. Ejecutar el servicio

**Modo desarrollo (con hot-reload):**
```bash
uvicorn src.main:app --reload --port 9000
```

**Modo producción:**
```bash
uvicorn src.main:app --host 0.0.0.0 --port 9000 --workers 4
```

### 4. Verificar que funciona

```bash
# Health check
curl http://127.0.0.1:9000/health
```

Deberías ver:
```json
{
  "status": "ok",
  "service": "ocr-service",
  "version": "1.0.0"
}
```

## 📡 API Endpoints

### `GET /health`

Health check del servicio.

**Response:**
```json
{
  "status": "ok",
  "service": "ocr-service",
  "version": "1.0.0"
}
```

### `POST /ocr/process`

Procesa un ticket PDF y extrae información.

**Request:**
```json
{
  "ticket_id": "550e8400-e29b-41d4-a716-446655440000",
  "file_name": "ticket_mercadona.pdf",
  "file_content_b64": "JVBERi0xLjQKJeLjz9MKMy4..."
}
```

**Response (éxito):**
```json
{
  "ticket_id": "550e8400-e29b-41d4-a716-446655440000",
  "raw_text": "TICKET DE PRUEBA...",
  "numero_factura": "TEST-000-000000",
  "fecha": "01/01/2025",
  "total": 12.34
}
```

**Response (error):**
```json
{
  "detail": "No se pudo procesar el PDF: PDF corrupto"
}
```

## 🧪 Testing Manual

### Con curl (Windows)

```powershell
# 1. Convertir PDF a base64 (PowerShell)
$pdfPath = "/ruta/a/ticket-sintetico.pdf"
$bytes = [System.IO.File]::ReadAllBytes($pdfPath)
$base64 = [System.Convert]::ToBase64String($bytes)

# 2. Crear JSON
$json = @{
    ticket_id = "test-123"
    file_name = "ticket_test.pdf"
    file_content_b64 = $base64
} | ConvertTo-Json

# 3. Enviar request
Invoke-RestMethod -Uri "http://127.0.0.1:9000/ocr/process" -Method POST -Body $json -ContentType "application/json"
```

### Con Python

```python
import base64
import requests

# Leer PDF y convertir a base64
with open("/ruta/a/ticket-sintetico.pdf", "rb") as f:
    file_content_b64 = base64.b64encode(f.read()).decode("utf-8")

# Enviar request
response = requests.post(
    "http://127.0.0.1:9000/ocr/process",
    json={
        "ticket_id": "test-123",
        "file_name": "ticket_test.pdf",
        "file_content_b64": file_content_b64
    }
)

print(response.json())
```

## 🗂️ Estructura del Proyecto

```
ocr-service/
├── requirements.txt          # Dependencias Python
├── README.md                 # Este archivo
└── src/
    ├── __init__.py
    ├── main.py               # FastAPI app
    ├── models.py             # Pydantic schemas
    ├── constants.py          # Regex patterns
    └── services/
        ├── __init__.py
        └── pdf_parser.py     # Lógica de extracción
```

## 🔍 Logging

El servicio usa **Loguru** para logging detallado:

```
2025-10-25 14:32:10 | INFO     | Processing ticket: test-123 | File: ticket_test.pdf
2025-10-25 14:32:10 | INFO     | PDF decoded. Size: 37045 bytes
2025-10-25 14:32:10 | INFO     | PDF opened successfully. Pages: 1
2025-10-25 14:32:10 | INFO     | Page 1: 1523 characters extracted
2025-10-25 14:32:10 | INFO     | ✓ Invoice number detected: TEST-000-000000
2025-10-25 14:32:10 | INFO     | ✓ Date detected: 01/01/2025
2025-10-25 14:32:10 | INFO     | ✓ Total detected: 12.34 €
2025-10-25 14:32:10 | SUCCESS  | ✅ Ticket processed successfully
```

## 🐛 Troubleshooting

### Error: "ModuleNotFoundError: No module named 'src'"

**Solución**: Ejecutar desde el directorio `ocr-service/`:
```bash
cd ocr-service
uvicorn src.main:app --reload --port 9000
```

### Error: "PDF corrupto o inválido"

**Posibles causas**:
- El PDF está dañado
- El base64 no está correctamente codificado
- El PDF es una imagen escaneada (no tiene texto extraíble)

**Solución para PDFs escaneados**: En futuras versiones se implementará OCR con Tesseract.

### Error: "Campos no detectados (null)"

**Causa**: El PDF no tiene el formato esperado de Mercadona.

**Solución**: Verificar que el PDF sea realmente de Mercadona y tenga los campos:
- "FACTURA SIMPLIFICADA: XXXX-XXX-XXXXXX"
- Fecha en formato DD/MM/YYYY
- "TOTAL (€) XX,XX"

## 🚧 Roadmap

### Fase 1 (MVP - Actual)
- ✅ Extracción de texto de PDFs nativos
- ✅ Detección de número de factura, fecha y total
- ✅ API REST con FastAPI

### Fase 2 (Próximas mejoras)
- [ ] OCR con Tesseract para PDFs escaneados
- [ ] Extracción de productos individuales
- [ ] Detección de categorías de productos
- [ ] Extracción de desglose de IVA
- [ ] Tests unitarios (pytest)

### Fase 3 (Producción)
- [ ] Dockerización
- [ ] Procesamiento asíncrono (cola de mensajes)
- [ ] Caché de resultados
- [ ] Métricas y monitoring
- [ ] Rate limiting

## 📚 Referencias

- [FastAPI Docs](https://fastapi.tiangolo.com/)
- [pdfplumber Docs](https://github.com/jsvine/pdfplumber)
- [Pydantic Docs](https://docs.pydantic.dev/)
- [Loguru Docs](https://loguru.readthedocs.io/)

## 📝 Notas

- Este worker está diseñado para trabajar **únicamente con tickets de Mercadona**.
- La extracción depende de patrones regex específicos del formato de Mercadona.
- Para otros supermercados, se necesitarán nuevos patrones y heurísticas.

---

**Última actualización**: 25/10/2025
**Versión**: 1.0.0
**Autor**: Juan Carlos
