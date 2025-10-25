"""
Script de prueba para el worker OCR.

Itera por todos los PDF disponibles en la carpeta ../docs,
los envía al endpoint /process-ticket y muestra por consola
toda la información extraída.
"""

import base64
import json
import sys
from pathlib import Path
from typing import Iterable

import httpx

BASE_DIR = Path(__file__).resolve().parent
PDF_DIRS = [BASE_DIR.parent / "docs"]
API_URL = "http://127.0.0.1:9000/process-ticket"


def iter_pdf_files() -> Iterable[Path]:
    """Devuelve todos los PDF encontrados en las carpetas configuradas."""
    for directory in PDF_DIRS:
        if not directory.exists():
            continue
        for pdf_path in sorted(directory.glob("*.pdf")):
            if pdf_path.is_file():
                yield pdf_path


def pdf_to_base64(pdf_path: Path) -> str:
    """Convierte un PDF a base64."""
    with pdf_path.open("rb") as fh:
        return base64.b64encode(fh.read()).decode("utf-8")


def print_ticket_response(data: dict) -> None:
    """Muestra por consola toda la informacion estructurada del ticket."""
    print("=" * 80)
    print(f"Ticket ID:        {data['ticket_id']}")
    print(f"Numero Factura:   {data.get('numero_factura')}")
    print(f"Fecha (legacy):   {data.get('fecha')}")
    print(f"Fecha/Hora:       {data.get('fecha_hora')}")
    print(f"Tienda:           {data.get('tienda')}")
    print(f"Ubicacion:        {data.get('ubicacion')}")
    print(f"Metodo Pago:      {data.get('metodo_pago')}")
    print(f"Numero Operacion: {data.get('numero_operacion')}")
    print(f"Total:            {data.get('total')}")

    iva_desglose = data.get("iva_desglose", [])
    if iva_desglose:
        print("\nDesglose IVA:")
        for item in iva_desglose:
            print(
                f"  - {item.get('porcentaje')}% | "
                f"Base: {item.get('base_imponible')} | Cuota: {item.get('cuota')}"
            )
    else:
        print("\nDesglose IVA:     no detectado")

    productos = data.get("productos", [])
    if productos:
        print("\nProductos detectados:")
        for prod in productos:
            print(
                f"  - {prod.get('nombre')} | "
                f"cantidad={prod.get('cantidad')} {prod.get('unidad')} | "
                f"p.unit={prod.get('precio_unitario')} | "
                f"p.total={prod.get('precio_total')} | "
                f"IVA%={prod.get('iva_porcentaje')} | "
                f"IVA importe={prod.get('iva_importe')} | "
                f"descuento={prod.get('descuento')}"
            )
    else:
        print("\nProductos detectados: ninguno")

    print("\nTexto completo del ticket:")
    print("-" * 80)
    print(data.get("raw_text", "").strip())
    print("-" * 80)
    print("=" * 80)


def main() -> None:
    """Ejecuta las pruebas contra el worker OCR."""
    pdf_files = list(iter_pdf_files())
    if not pdf_files:
        print("[WARN] No se encontraron archivos PDF en las carpetas configuradas.")
        for directory in PDF_DIRS:
            print(f"        - {directory}")
        sys.exit(0)

    print("[INFO] Se encontraron los siguientes PDF:")
    for pdf in pdf_files:
        print(f"  - {pdf}")

    for pdf_path in pdf_files:
        ticket_id = f"test-{pdf_path.stem.replace(' ', '-').lower()}"
        print("\n" + "#" * 80)
        print(f"[INFO] Procesando {pdf_path.name} (ticket_id={ticket_id})")

        payload = {
            "ticket_id": ticket_id,
            "file_name": pdf_path.name,
            "pdf_b64": pdf_to_base64(pdf_path),
        }

        try:
            response = httpx.post(API_URL, json=payload, timeout=30.0)
            response.raise_for_status()
        except httpx.RequestError as exc:
            print(f"[ERROR] No se pudo conectar con el worker: {exc}")
            continue
        except httpx.HTTPStatusError as exc:
            print(f"[ERROR] HTTP {exc.response.status_code}: {exc.response.text}")
            continue

        data = response.json()
        print_ticket_response(data)

        output_path = BASE_DIR / f"test_response_{ticket_id}.json"
        with output_path.open("w", encoding="utf-8") as fh:
            json.dump(data, fh, indent=2, ensure_ascii=False)
        print(f"[OK] Respuesta guardada en {output_path}")


if __name__ == "__main__":
    main()

