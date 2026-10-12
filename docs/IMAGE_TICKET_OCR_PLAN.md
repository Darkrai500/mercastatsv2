# Fotos de tickets en papel

Última actualización: 12/10/2026 · Estado: implementado (rama `feat/ticket-foto-ocr`)

## 1. Objetivo

Además del PDF del ticket digital, poder subir la **foto de un ticket en papel** de Mercadona, con todo lo que implica una foto: perspectiva, pliegues, sombras, tinta térmica gastada, desenfoque, fotos de iPhone en HEIC y archivos de varios megas.

Restricción de partida: **el camino de los PDF no cambia**. El parser de PDF ya funcionaba bien; las fotos van por un camino aparte y solo comparten la ingesta en base de datos.

## 2. Decisión: modelo de visión vía OpenRouter, Tesseract de respaldo

Se evaluaron los dos enfoques con una foto real de un ticket de 13 productos (no incluida en el repositorio) y tres versiones degradadas de la misma foto (girada con perspectiva, oscura con sombra, borrosa a baja resolución con JPEG comprimido).

**Tesseract + OpenCV** (14 combinaciones de recorte, corrección de luz, binarización, escala y modo de página). La mejor configuración leyó 7 de 13 productos (menos de la mitad del importe), no leyó el TOTAL en letra grande y perdió dígitos en cuatro importes (`4,5`, `3,7€`, `1,0$`, `1,5`). Con cuatro incógnitas los cuadres del ticket no bastan para reconstruirlos. Además es inestable: la similitud con el texto real iba de 0,03 a 0,76 según el reescalado.

**Modelos de visión** (36 llamadas por OpenRouter, 0,04 $ en total). "Perfecto" significa los 6 campos de cabecera, los 13 importes, las cantidades y los nombres (sin contar tildes) correctos:

| Modelo | Perfectos | Coste por foto | Latencia media |
|---|---|---|---|
| `anthropic/claude-haiku-5.5` | 6/6 | 0,0011 $ | 7,6 s |
| `google/gemini-3.1-flash-lite` | 5/6 | 0,0024 $ | 4,0 s |
| `qwen/qwen3.7-flash` | 3/6 | 0,0004 $ | 33 s |
| `google/gemini-2.5-flash-lite` | 3/6 (1 JSON inválido) | 0,0011 $ | 7,1 s |
| `openai/gpt-6-luna` | 2/6 | 0,0008 $ | 9,3 s |
| `openai/gpt-5-nano` | 0/6 | 0,0010 $ | 17 s |

Por defecto se usa Claude Haiku 5.5 con Gemini 3.1 Flash Lite de respaldo (`OCR_VISION_MODELS`). A ~1 $ cada 1.000 fotos el coste es despreciable para un uso doméstico. Sin `OPENROUTER_API_KEY` (o si ningún modelo responde) se usa Tesseract, con un aviso de que la lectura es poco fiable.

Fallos típicos observados, que el código cubre:
- en fotos borrosas los modelos pierden tildes (`SALMON`): al guardar, los nombres sin tildes se emparejan con el catálogo;
- algunos modelos meten la cantidad en el nombre (`1 PAN`): se quita solo si coincide con la cantidad comprada, porque hay productos como `12 HUEVOS GRANDES-L`;
- algún modelo devuelve el objeto dentro de una lista o un JSON roto: se desenvuelve o se pasa al siguiente modelo.

## 3. Flujo

```
Foto ─► /api/ocr/process ─► servicio Python
          (backend)            ├─ ¿foto por bytes mágicos? (JPEG/PNG/WEBP/HEIC)
                               │    sí ─► EXIF, RGB, ≤2000 px, JPEG ─► OpenRouter (JSON con esquema estricto)
                               │          └─ sin clave / fallo ─► Tesseract + OpenCV
                               │          + avisos de coherencia (suma ≠ total, IVA, campos que faltan)
                               └─ no ─► parser de PDF de siempre (sin cambios)
      ◄─ resumen completo
Revisión editable (frontend, solo fotos)
      ─► /api/ocr/confirm ─► valida (foto, importes coherentes) ─► catálogo + líneas repetidas ─► ingesta común
```

- **PDF:** misma respuesta que antes (sin `processing_profile` ni avisos nuevos), la misma revisión de solo lectura y el mismo guardado reprocesando el PDF. Un test compara la respuesta con la del parser de PDF directamente.
- **`/api/ocr/confirm`** solo admite fotos y guarda los datos revisados sin repetir el OCR. Comprueba lo que exigiría la base de datos (cantidad × precio unitario = importe, suma = total) para devolver un motivo legible en lugar de un error genérico.
- La foto original se guarda en `tickets_pdf`, como los PDF. La migración `0004_ticket_photos.sql` amplía la restricción de extensión a `jpg/jpeg/png/webp/heic/heif`.
- Límites: el cuerpo JSON de `/api/ocr` admite 16 MB (antes, 2 MB por defecto en axum) y el proxy nginx del frontend también (antes, 1 MB). La foto tiene un máximo de 10 MB.
- La lectura completa (todos los modelos) tiene un presupuesto de `OCR_VISION_TIMEOUT` (25 s), por debajo del timeout del backend (`INTELLIGENCE_TIMEOUT_SECS`, 30 s), para que siempre quede margen para Tesseract.
- Privacidad: la foto sale a OpenRouter y al proveedor del modelo; la interfaz lo indica. En los logs solo aparecen el modelo, la latencia, el coste y el número de productos.

## 4. Pendiente / ideas

- **PDF escaneados** (sin texto): hoy fallan como antes. Podrían rasterizarse y leerse como foto, pero eso cambiaría el camino de los PDF y se dejó fuera a propósito.
- **Reducir la foto en el navegador** antes de subirla (menos datos móviles y menos almacenamiento). HEIC no se puede decodificar en Chrome con canvas.
- **Vista previa de HEIC:** solo Safari la muestra; en el resto se ve un aviso.
- Detectado en el camino de PDF y **sin tocar** a propósito: el patrón de método de pago espera `TARJJETA` (errata), así que "TARJETA BANCARIA" escrito completo no se reconoce. Además, tras guardar un PDF la lista muestra "(0 prods)", porque la respuesta de ingesta trae `productos_insertados` y la interfaz cuenta `productos`.
