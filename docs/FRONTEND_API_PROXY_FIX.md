# Frontend API Proxy Fix

## Resumen
El frontend compilado para produccion estaba apuntando a `http://localhost:8000/api`,
lo que provocaba errores de CORS y fallo de red en el navegador de usuarios en el VPS.
Se cambio la base URL del cliente a ruta relativa y se ajusto el proxy de Nginx para
enrutar `/api/` hacia el contenedor del backend dentro de Docker.

## Problema
- El frontend buscaba la API en `localhost:8000`, que en el navegador del usuario
  apunta a su propia maquina, no al servidor.
- El contenedor del frontend estaba operativo, pero las llamadas a la API fallaban
  por CORS y NetworkError.

## Causa raiz
- URL base hardcodeada en el cliente WASM.
- El proxy de Nginx estaba apuntando a un host interno distinto al nombre real
  del contenedor del backend en el compose.

## Solucion aplicada
1. Usar `/api` como URL base del cliente para que el navegador use el mismo origen.
2. Configurar Nginx como reverse proxy hacia `mercastats_backend:8000`.

## Cambios de codigo
- `frontend/src/api/mod.rs`: `API_BASE_URL` ahora es `/api`.
- `frontend/nginx.conf`: `proxy_pass` ahora apunta a `http://mercastats_backend:8000/api/`.

## Verificacion sugerida
1. Reconstruir imagen del frontend.
2. Desplegar en VPS.
3. Abrir el sitio y comprobar que `/api/health` responde desde el navegador.

## Notas de despliegue
Ejemplo de flujo local:

```powershell
docker build -t mercastats-frontend:latest ./frontend
docker save -o frontend_fix.tar mercastats-frontend:latest
scp .\frontend_fix.tar debian@IP_VPS:/home/debian/
```

En el VPS:

```bash
docker load -i /home/debian/frontend_fix.tar
docker compose up -d
```
