"""Pruebas de la autenticación opcional entre servicios."""

import asyncio

import pytest
from fastapi import HTTPException

from main import require_internal_api_key


def test_api_key_no_se_exige_si_no_esta_configurada(monkeypatch) -> None:
    monkeypatch.delenv("INTELLIGENCE_API_KEY", raising=False)

    asyncio.run(require_internal_api_key(None))


def test_api_key_rechaza_valor_ausente_o_incorrecto(monkeypatch) -> None:
    monkeypatch.setenv("INTELLIGENCE_API_KEY", "clave-de-prueba")

    for provided in (None, "otra-clave"):
        with pytest.raises(HTTPException) as exc_info:
            asyncio.run(require_internal_api_key(provided))
        assert exc_info.value.status_code == 401


def test_api_key_acepta_valor_correcto(monkeypatch) -> None:
    monkeypatch.setenv("INTELLIGENCE_API_KEY", "clave-de-prueba")

    asyncio.run(require_internal_api_key("clave-de-prueba"))
