# Plan de Implementacion: OCR de Tickets en Imagen

Ultima actualizacion: 27/11/2025

## 1. Objetivo

Extender el pipeline de OCR para que procese tickets de Mercadona en formato imagen (JPG/PNG/WEBP/HEIC) con la misma calidad de extraccion que los PDFs. La solucion debe detectar tickets escaneados dentro de PDFs (sin texto embebido), aplicar preprocesado robusto, y devolver errores claros cuando no haya ticket interpretable.

## 2. Alcance

- IN: soporte de imagen en el worker Python; fallback OCR para PDFs escaneados; validaciones y mensajes de error; actualizacion de contrato entre backend y worker; ajustes menores en backend/frontend para nuevos campos y errores.
- OUT: entrenamiento de modelos ML, deduplicacion avanzada de productos, almacenamiento de la imagen original en BD.

## 3. Estado actual

- Frontend (`frontend/src/pages/upload.rs`) ya acepta imagenes y PDFs y envia base64 a `POST /ocr/process`.
- Backend (`backend/src/routes/ocr.rs`) valida `file_content_b64` (alias `pdf_b64`) y delega al servicio de inteligencia sin distinguir MIME.
- Worker (`ocr-service/src/services/pdf_parser.py`) solo maneja PDFs de texto via `pdfplumber`; falla si el PDF es un scan o si se envia una imagen directa.
- Errores: se retorna 422 cuando no hay texto en PDF, pero no hay clasificadores de causa ni manejo para imagenes.

## 4. Consideraciones clave

- Calidad variable: fotos inclinadas, sombras, pliegues, desenfoque, fondos oscuros.
- Formatos: JPG/PNG nativos, WEBP de moviles, HEIC/HEIF de iOS; PDFs que contienen solo una imagen.
- Rendimiento: limitar resolucion max al preprocesar para evitar timeouts en Tesseract.
- Seguridad: rechazar archivos que no sean imagen/PDF al validar el buffer, no solo la extension.

## 5. Plan tecnico

### 5.1 Worker Python (ocr-service)

1) Nuevas dependencias: `pytesseract`, `Pillow` (+ `pillow-heif` para HEIC/HEIF), `opencv-python-headless` para preprocesado (deskew/denoise), `pdf2image` opcional si necesitamos rasterizar paginas sin `page.to_image()`. Documentar requisito del binario Tesseract en README/dev.js.
2) Deteccion de tipo: inspeccionar cabecera del buffer (magic bytes) para decidir `source_type = pdf|image`. En PDF, si `extract_text_from_pdf` retorna poco texto, rasterizar cada pagina y pasar por el pipeline de imagen.
3) Pipeline de imagen:
   - Cargar con Pillow/pillow-heif, normalizar orientacion EXIF, convertir a RGB -> escala de grises.
   - Preprocesado: resize max 2000px lado largo, filtro de mediana, threshold adaptativo, inversion opcional; deskew basico con OpenCV (Hough/angle).
   - Ejecutar `pytesseract` con `lang=spa` y modo OEM/PSM adecuado (ej. PSM 6 o 4); retry con variantes (crudo, binarizado).
   - Post-procesar texto (normalizar espacios, upper), reutilizar regex existentes.
4) Reutilizar parser: pasar el texto OCR al mismo extractor de campos (`extract_numero_factura`, `extract_products`, etc.) para mantener heuristicas unificadas.
5) Validaciones y errores:
   - Si texto < umbral o no contiene minimos (`MERCADONA`, `FACTURA`, `TOTAL`), lanzar `TicketNotDetectedError`.
   - Si OCR devuelve texto pero sin campos clave, devolver 422 con mensaje orientativo.
   - Añadir enum/codigos: `UNSUPPORTED_FORMAT`, `EMPTY_TEXT`, `NO_TICKET_DETECTED`, `OCR_TIMEOUT`, `PARSER_ERROR`.
6) Observabilidad: log de `source_type`, tiempo de OCR, longitud de texto, numero de productos, motivo de rechazo. Truncar texto en logs.

### 5.2 Contrato API worker <-> backend

- Request: mantener `file_content_b64`, añadir opcional `mime_type` y `source_type_detected` en logs; permitir alias `pdf_b64` por compatibilidad.
- Response: añadir campos opcionales `processing_profile` (`pdf-text`, `pdf-ocr`, `image-ocr`) y `warnings` (ej. "texto escaso", "deskew aplicado").
- Errores: mapear a HTTP 415 para formatos no soportados, 422 para ticket no detectado o OCR insuficiente, 504/500 para timeouts o fallos internos.

### 5.3 Backend (Rust)

- Validar MIME segun cabecera del archivo y nombre; rechazar extensiones peligrosas antes de enviar al worker.
- Propagar `mime_type` en `OcrProcessTicketRequest`; mapear codigos de error nuevos a respuestas claras para frontend.
- Ajustar mensajes de trazas para indicar pipeline usado y motivo de rechazo.

### 5.4 Frontend

- Mantener soporte de seleccion/preview de imagen. Ajustar mensajes de error para los nuevos codigos (no ticket, formato no soportado, imagen ilegible).
- Mostrar recomendacion breve cuando llegue `warnings` (por ejemplo, pedir foto mas nítida).

### 5.5 Testing

- Fixtures: crear imagenes de ejemplo (nítida, borrosa, inclinada, recortada) y un PDF escaneado. Guardar en `docs/Tickets/fixtures`.
- Unit tests en Python para:
  - Deteccion de MIME.
  - Pipeline de preprocesado (deskew, binarizacion) retornando texto no vacio.
  - Errores por texto insuficiente o formato no soportado.
- Integration: test FastAPI `/ocr/process` con imagen valida, PDF escaneado y casos de error (formato, texto vacio).
- Backend: test de mapeo de errores del worker y validacion de MIME en `TicketProcessPayload`.

## 6. Entregables

- Codigo del worker con pipeline de imagen y fallback OCR para PDFs sin texto.
- Nuevos codigos de error y mensajes tipados en worker y backend.
- Actualizacion de `requirements.txt`, README del servicio, y guia de ejecucion (instalacion de Tesseract).
- Fixtures de prueba y suite de tests automatizados para imagenes.
- Ajustes de frontend para surfacing de errores/warnings.

## 7. Riesgos y mitigacion

- Dependencia externa Tesseract: documentar instalacion y fallback a texto vacio con error claro si el binario no existe.
- Rendimiento: limitar resolucion y tiempo de OCR; timeout configurable.
- Formatos raros (HEIC/WEBP grandes): validar y comprimir previo al OCR.

## 8. Preguntas abiertas

1) Disponemos de Tesseract instalado en el entorno CI/dev o se necesita contenedor con el binario?  
2) Limite de tamaño aceptado para imagenes (MB/resolucion) antes de rechazar?  
3) Preferencia de idioma OCR (solo `spa` o añadir `eng` como fallback por marcas/visa)?  
4) Deseamos devolver el texto completo al frontend o solo el resumen actual?
