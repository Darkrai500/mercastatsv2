"""
Comprobaciones de coherencia de un ticket ya leído.

Un ticket de Mercadona trae información redundante (suma de líneas = total, bases + cuotas
de IVA = total), así que un dígito mal leído casi siempre se delata. Los avisos no bloquean:
se muestran en la revisión para que el usuario corrija antes de guardar.
"""

from typing import List

from services.pdf_parser import ParsedTicket

TOLERANCIA_EUROS = 0.01


def _euros(value: float) -> str:
    return f"{value:.2f}".replace(".", ",") + " €"


def check_ticket(ticket: ParsedTicket) -> List[str]:
    warnings: List[str] = []

    if not ticket.numero_factura:
        warnings.append("No se ha leído el número de factura.")
    if not ticket.fecha:
        warnings.append("No se ha leído la fecha.")
    elif not ticket.fecha_hora:
        warnings.append("No se ha leído la hora.")
    if ticket.total is None:
        warnings.append("No se ha leído el total.")
    if not ticket.productos:
        warnings.append("No se ha detectado ningún producto.")

    if ticket.total is not None and ticket.productos:
        suma = round(sum(p.precio_total for p in ticket.productos), 2)
        if abs(suma - ticket.total) > TOLERANCIA_EUROS:
            warnings.append(
                f"La suma de los productos ({_euros(suma)}) no coincide con el total ({_euros(ticket.total)})."
            )

    if ticket.total is not None and ticket.iva_desglose:
        suma_iva = round(sum(i.base_imponible + i.cuota for i in ticket.iva_desglose), 2)
        if abs(suma_iva - ticket.total) > 0.02 * len(ticket.iva_desglose):
            warnings.append("El desglose de IVA no cuadra con el total.")

    return warnings
