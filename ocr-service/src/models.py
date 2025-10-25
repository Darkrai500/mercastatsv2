"""
Modelos Pydantic para request/response del worker OCR.

Define los contratos de datos entre el backend Rust y el worker Python.
"""

from datetime import datetime
from typing import List, Optional

from pydantic import BaseModel, Field


class ProcessTicketRequest(BaseModel):
    """
    Request para procesar un ticket en formato PDF.

    Attributes:
        ticket_id: ID temporal/UUID generado por el backend para correlacion
        file_name: Nombre del archivo PDF original
        pdf_b64: Contenido del PDF codificado en base64
    """

    ticket_id: str = Field(..., description="ID provisional del ticket (UUID)")
    file_name: str = Field(..., description="Nombre del archivo PDF")
    pdf_b64: str = Field(..., description="Contenido PDF en base64")

    class Config:
        json_schema_extra = {
            "example": {
                "ticket_id": "550e8400-e29b-41d4-a716-446655440000",
                "file_name": "ticket_mercadona_2023.pdf",
                "pdf_b64": "JVBERi0xLjQKJeLjz9MKMy..."
            }
        }


class TicketProduct(BaseModel):
    """Producto detectado en el ticket."""

    nombre: str = Field(..., description="Nombre del producto tal y como aparece en el ticket")
    cantidad: float = Field(..., description="Cantidad comprada (unidades o peso)")
    unidad: str = Field(..., description="Unidad de medida normalizada (unidad, kg, l, etc.)")
    precio_unitario: float = Field(..., description="Precio unitario o por kilo/litro")
    precio_total: float = Field(..., description="Importe total pagado por el producto")
    descuento: float = Field(0.0, description="Importe de descuento aplicado al producto")
    iva_porcentaje: float = Field(0.0, description="Porcentaje de IVA asociado al producto")
    iva_importe: float = Field(0.0, description="Importe de IVA estimado para el producto")


class IvaBreakdownModel(BaseModel):
    """Resumen del IVA encontrado en el ticket."""

    porcentaje: float = Field(..., description="Porcentaje de IVA")
    base_imponible: float = Field(..., description="Base imponible declarada en el ticket")
    cuota: float = Field(..., description="Cuota de IVA para la base imponible")


class ProcessTicketResponse(BaseModel):
    """
    Respuesta despues de procesar un ticket.

    Contiene tanto el texto crudo como los campos estructurados necesarios
    para poblar las tablas `compras` y `compras_productos`.
    """

    ticket_id: str = Field(..., description="ID del ticket procesado")
    raw_text: str = Field(..., description="Texto completo extraido")
    numero_factura: Optional[str] = Field(None, description="Numero de factura (XXXX-XXX-XXXXXX)")
    fecha: Optional[str] = Field(None, description="Fecha del ticket (dd/mm/yyyy) [compatibilidad]")
    fecha_hora: Optional[datetime] = Field(
        None,
        description="Fecha y hora del ticket en formato ISO 8601",
    )
    total: Optional[float] = Field(None, description="Total en euros")
    tienda: Optional[str] = Field(None, description="Nombre de la tienda o razon social")
    ubicacion: Optional[str] = Field(None, description="Direccion completa de la tienda")
    metodo_pago: Optional[str] = Field(None, description="Metodo de pago detectado")
    numero_operacion: Optional[str] = Field(None, description="Numero de operacion o referencia")
    productos: List[TicketProduct] = Field(default_factory=list, description="Productos detectados")
    iva_desglose: List[IvaBreakdownModel] = Field(
        default_factory=list,
        description="Desglose de IVA encontrado en el ticket",
    )

    class Config:
        json_schema_extra = {
            "example": {
                "ticket_id": "550e8400-e29b-41d4-a716-446655440000",
                "raw_text": "MERCADONA, S.A...",
                "numero_factura": "2831-021-575287",
                "fecha": "10/08/2023",
                "fecha_hora": "2023-08-10T19:46:00",
                "total": 52.11,
                "tienda": "MERCADONA, S.A.",
                "ubicacion": "C/ PORTUGAL 37, 28943 FUENLABRADA",
                "metodo_pago": "Tarjeta bancaria",
                "numero_operacion": "367328",
                "productos": [
                    {
                        "nombre": "12 HUEVOS GRANDES-L",
                        "cantidad": 1,
                        "unidad": "unidad",
                        "precio_unitario": 2.2,
                        "precio_total": 2.2,
                        "descuento": 0.0,
                        "iva_porcentaje": 10.0,
                        "iva_importe": 0.2
                    }
                ],
                "iva_desglose": [
                    {"porcentaje": 10.0, "base_imponible": 22.7, "cuota": 2.27},
                    {"porcentaje": 21.0, "base_imponible": 11.94, "cuota": 2.51},
                    {"porcentaje": 0.0, "base_imponible": 12.69, "cuota": 0.0}
                ]
            }
        }


class HealthResponse(BaseModel):
    """
    Respuesta del endpoint de health check.
    """

    status: str = Field(default="ok")
    service: str = Field(default="ocr-service")
    version: str = Field(default="1.0.0")
