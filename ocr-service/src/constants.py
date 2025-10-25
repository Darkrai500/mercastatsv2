"""
Constantes y patrones regex para parsing de tickets de Mercadona.

Define los patrones de expresiones regulares utilizados para extraer
información estructurada de los tickets de compra.
"""

import re

# ============================================================================
# PATRONES REGEX PARA MERCADONA
# ============================================================================

# Número de factura: formato XXXX-XXX-XXXXXX
# Ejemplo: "FACTURA SIMPLIFICADA: 2831-021-575287"
PATTERN_NUMERO_FACTURA = re.compile(
    r"FACTURA\s+SIMPLIFICADA:\s*(\d{4}-\d{3}-\d{6})",
    re.IGNORECASE
)

# Fecha: formato DD/MM/YYYY
# Ejemplo: "10/08/2023 19:46"
# Incluye versi�n con hora en minutos
PATTERN_FECHA_HORA = re.compile(
    r"\b(\d{2}/\d{2}/\d{4})\s+(\d{2}:\d{2})\b"
)

# Captura solo la fecha, ignorando la hora (fallback)
PATTERN_FECHA = re.compile(
    r"\b(\d{2}/\d{2}/\d{4})\b"
)

# Total del ticket: formato "TOTAL (€) XX,XX"
# Ejemplo: "TOTAL (€) 52,11"
# Nota: Mercadona usa coma (,) como separador decimal
PATTERN_TOTAL = re.compile(
    r"TOTAL\s*\((?:€|�)\)\s*([0-9]+,[0-9]{2})",
    re.IGNORECASE
)

# Alternativa: si el total aparece como "TOTAL 52,11"
PATTERN_TOTAL_ALT = re.compile(
    r"TOTAL\s+([0-9]+,[0-9]{2})",
    re.IGNORECASE
)

# ============================================================================
# PATRONES ADICIONALES (para futuras mejoras)
# ============================================================================

# Metodo de pago habitual
PATTERN_METODO_PAGO = re.compile(
    r"(TARJ(?:\.|JETA)\s+BANCARIA|EFECTIVO|BIZUM|MASTERCARD|VISA|AMEX)",
    re.IGNORECASE
)

# Numero de operacion que suele aparecer junto a la fecha
PATTERN_NUMERO_OPERACION = re.compile(
    r"OP:\s*(\d+)",
    re.IGNORECASE
)

# Línea de producto: cantidad, descripción, precio unitario (opcional), importe
# Ejemplo: "1 12 HUEVOS GRANDES-L 2,20"
# Ejemplo con precio por kg: "1 PIMIENTO VERDE\n0,228 kg 2,49 €/kg 0,57"
PATTERN_PRODUCTO = re.compile(
    r"^(\d+)\s+(.+?)\s+([0-9]+,[0-9]{2})$",
    re.MULTILINE
)

# Detalle de producto pesado en la línea siguiente
PATTERN_PRODUCTO_PESADO_DETALLE = re.compile(
    r"^(?P<peso>\d+,\d{2,3})\s*(?P<unidad>kg|g|l|ml)\s+(?P<precio_unitario>\d+,[0-9]{2}).*?(?P<importe>\d+,[0-9]{2})$",
    re.IGNORECASE
)

# IVA: captura el desglose de IVA
# Ejemplo: "10% 22,70 2,27"
PATTERN_IVA = re.compile(
    r"(\d+)%\s+([0-9]+,[0-9]{2})\s+([0-9]+,[0-9]{2})"
)

# Dirección de la tienda
# Ejemplo: "C/ PORTUGAL 37\n28943 FUENLABRADA"
PATTERN_DIRECCION = re.compile(
    r"C/\s+(.+?)\n(\d{5}\s+\w+)",
    re.IGNORECASE
)

# CIF de Mercadona
PATTERN_CIF = re.compile(
    r"MERCADONA,\s*S\.A\.\s*(A-\d{8})",
    re.IGNORECASE
)

# ============================================================================
# CONFIGURACIÓN
# ============================================================================

# Número máximo de caracteres de raw_text para logging
MAX_RAW_TEXT_PREVIEW = 500

# Timeout para procesamiento de un PDF (segundos)
PROCESSING_TIMEOUT = 30
