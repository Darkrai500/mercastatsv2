#!/usr/bin/env bash
# Lanzador de Mercastats para macOS: un solo comando deja todo funcionando.
#
#   ./run.sh           desarrollo: PostgreSQL en Docker + servicio Python + API Rust + frontend
#                      con recarga automática  ->  http://localhost:3000
#   ./run.sh docker    todo el stack en contenedores (docker compose up --build)
#   ./run.sh test      tests de Python y del backend Rust
#   ./run.sh setup     solo prepara el entorno (.env, venv, trunk...) sin arrancar nada
#   ./run.sh stop      detiene la base de datos (los datos se conservan)
#
# Es idempotente: la primera vez instala lo que falte; las siguientes tarda segundos.
# Variables: NO_OPEN=1 no abre el navegador al terminar de compilar el frontend.

set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"

if [[ -t 1 ]]; then
  BOLD=$'\033[1m'; GREEN=$'\033[32m'; YELLOW=$'\033[33m'; RED=$'\033[31m'; RESET=$'\033[0m'
else
  BOLD=""; GREEN=""; YELLOW=""; RED=""; RESET=""
fi
say()  { printf '%s==>%s %s\n' "$GREEN" "$RESET" "$*"; }
warn() { printf '%s!!%s  %s\n' "$YELLOW" "$RESET" "$*" >&2; }
die()  { printf '%sxx%s  %s\n' "$RED" "$RESET" "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

[[ "$(uname -s)" == "Darwin" ]] || warn "Pensado para macOS; en otros sistemas puede necesitar ajustes."

# Valor de una variable de .env sin ejecutar el fichero (acepta comentarios y valores raros).
env_get() {
  local value
  value="$(grep -E "^$1=" .env 2>/dev/null | tail -1 | cut -d= -f2- || true)"
  printf '%s' "${value:-$2}"
}

# ---------------------------------------------------------------------------
# Preparación
# ---------------------------------------------------------------------------

# Clave con la que el backend cifra los tokens de Google. Se añade también a los .env que ya
# existían, para que activar el inicio de sesión con Google solo requiera las credenciales.
ensure_token_key() {
  [[ -n "$(env_get TOKEN_ENCRYPTION_KEY '')" ]] && return
  local key
  key="$(openssl rand -base64 32)"
  if grep -q '^TOKEN_ENCRYPTION_KEY=' .env; then
    sed -i.bak "s|^TOKEN_ENCRYPTION_KEY=.*|TOKEN_ENCRYPTION_KEY=${key}|" .env
    rm -f .env.bak
  else
    printf '\n# Clave AES-256 para cifrar los tokens de Google en la BD (generada por run.sh)\nTOKEN_ENCRYPTION_KEY=%s\n' "$key" >> .env
  fi
  say "Generada TOKEN_ENCRYPTION_KEY en .env"
}

ensure_env() {
  if [[ -f .env ]]; then ensure_token_key; return; fi
  say "Creando .env con secretos aleatorios (solo la primera vez)"
  local db_password jwt_secret
  db_password="$(openssl rand -hex 16)"
  jwt_secret="$(openssl rand -base64 32)"
  # '|' como delimitador: el base64 puede contener '/'.
  sed -e "s|change_me_to_a_local_database_password|${db_password}|g" \
      -e "s|^JWT_SECRET=.*|JWT_SECRET=${jwt_secret}|" \
      .env.example > .env
  chmod 600 .env
  ensure_token_key
}

ensure_docker() {
  have docker || die "Falta Docker. Instálalo con:  brew install --cask orbstack   (o Docker Desktop)"
  docker info >/dev/null 2>&1 && return
  say "Arrancando el motor de Docker…"
  if   [[ -d /Applications/OrbStack.app ]]; then open -a OrbStack
  elif [[ -d /Applications/Docker.app   ]]; then open -a Docker
  else die "Docker no responde y no encuentro OrbStack ni Docker Desktop en /Applications."; fi
  for _ in $(seq 1 60); do
    docker info >/dev/null 2>&1 && return
    sleep 2
  done
  die "Docker no respondió en 2 minutos. Ábrelo a mano y vuelve a lanzar ./run.sh"
}

# ¿Hay algo escuchando en este puerto del loopback? `nc` ve también los procesos de otros
# usuarios (p. ej. un PostgreSQL nativo del sistema); `lsof` no.
port_busy() {
  nc -z 127.0.0.1 "$1" >/dev/null 2>&1 || nc -z ::1 "$1" >/dev/null 2>&1
}

set_db_port() {
  local old new="$1"
  old="$(env_get POSTGRES_PORT 5432)"
  sed -i.bak -e "s|^POSTGRES_PORT=.*|POSTGRES_PORT=${new}|" \
             -e "s|@localhost:${old}/|@localhost:${new}/|" .env
  rm -f .env.bak
}

# Comprueba que `localhost:PUERTO` llega de verdad a NUESTRA base de datos. Docker/OrbStack
# publican el puerto aunque otro PostgreSQL ya lo ocupe, y entonces el backend acabaría
# hablando con el servidor equivocado ("password authentication failed").
verify_db_login() {
  local port
  port="$(env_get POSTGRES_PORT 5432)"
  PGPASSWORD="$(env_get POSTGRES_PASSWORD '')" .venv/bin/python - \
    "$(env_get POSTGRES_USER mercastats_app)" "$(env_get POSTGRES_DB mercastats)" "$port" <<'PY' \
    || die "localhost:${port} no responde nuestra base de datos: otro PostgreSQL ocupa ese puerto. Ejecuta  ./run.sh stop  y vuelve a lanzar (elegiré un puerto libre)."
import sys
import psycopg2

user, db, port = sys.argv[1:4]
try:
    psycopg2.connect(host="127.0.0.1", port=int(port), user=user, dbname=db, connect_timeout=5).close()
except Exception as exc:
    sys.stderr.write(str(exc).strip().splitlines()[0] + "\n")
    sys.exit(1)
PY
}

# docker-compose solo ejecuta los scripts de initdb con el volumen vacío, así que una base de
# datos ya existente no recibe las migraciones nuevas. Las posteriores a la 0002 son
# idempotentes y se aplican aquí en cada arranque.
apply_migrations() {
  local file user db
  user="$(env_get POSTGRES_USER mercastats_app)"; db="$(env_get POSTGRES_DB mercastats)"
  for file in backend/migrations/0*.sql; do
    case "$(basename "$file")" in 0001_*|0002_*) continue ;; esac
    docker exec -i -e PGOPTIONS='-c client_min_messages=warning' mercastats_db \
      psql -v ON_ERROR_STOP=1 -q -U "$user" -d "$db" < "$file" >/dev/null \
      || die "No se pudo aplicar la migración $(basename "$file")"
  done
}

ensure_db() {
  ensure_docker
  local running port free
  running="$(docker inspect -f '{{.State.Running}}' mercastats_db 2>/dev/null || echo false)"
  port="$(env_get POSTGRES_PORT 5432)"
  if [[ "$running" != "true" ]] && port_busy "$port"; then
    for free in 5433 5434 5435 5436 5437; do port_busy "$free" || break; done
    port_busy "$free" && die "No encuentro un puerto libre para PostgreSQL (probé 5433-5437)."
    warn "El puerto $port ya lo usa otro servidor (¿un PostgreSQL instalado en el Mac?). Uso el $free para la BD de Mercastats."
    set_db_port "$free"
  fi
  say "Levantando PostgreSQL (Docker)"
  docker compose up -d db
  local status
  for _ in $(seq 1 45); do
    status="$(docker inspect -f '{{.State.Health.Status}}' mercastats_db 2>/dev/null || true)"
    [[ "$status" == "healthy" ]] && { verify_db_login; apply_migrations; return; }
    sleep 2
  done
  die "PostgreSQL no llegó a estar sano. Revisa:  docker compose logs db"
}

ensure_python() {
  local requirements=intelligence-service/requirements.txt stamp=.venv/.requirements.sha sum
  if [[ ! -x .venv/bin/python ]]; then
    say "Creando entorno virtual de Python (.venv)"
    if have uv; then
      uv venv .venv --python 3.12 -q 2>/dev/null || uv venv .venv -q
    else
      have python3 || die "Falta Python 3:  brew install python"
      python3 -m venv .venv
    fi
  fi
  sum="$(shasum -a 256 "$requirements" | cut -d' ' -f1)"
  if [[ "$(cat "$stamp" 2>/dev/null || true)" != "$sum" ]]; then
    say "Instalando dependencias de Python"
    if have uv; then
      uv pip install -q --python .venv/bin/python -r "$requirements" pytest httpx
    else
      .venv/bin/python -m pip install -q -r "$requirements" pytest httpx
    fi
    echo "$sum" > "$stamp"
  fi
  have tesseract || warn "Sin tesseract no se podrán leer tickets en imagen (PDF sí):  brew install tesseract"
}

ensure_rust() {
  have cargo  || die "Falta Rust:  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  have rustup || die "Falta rustup (necesario para el objetivo WebAssembly)."
  if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
    say "Añadiendo el objetivo wasm32-unknown-unknown"
    rustup target add wasm32-unknown-unknown
  fi
  if ! have trunk; then
    say "Instalando trunk (solo la primera vez)"
    if have brew; then brew install trunk; else cargo install --locked trunk; fi
  fi
}

ensure_node() {
  have node || die "Falta Node.js (lo usa dev.js):  brew install node"
}

check_port() {
  local port="$1" label="$2" pid
  pid="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null | head -1 || true)"
  if [[ -n "$pid" ]]; then
    die "El puerto $port ($label) ya lo usa $(ps -p "$pid" -o comm= 2>/dev/null || echo '?') (PID $pid). Ciérralo y reintenta."
  fi
}

# Abre el navegador cuando el frontend responde (la primera compilación tarda minutos).
# Deja el PID del vigilante en $WATCHER para pararlo al salir.
open_when_ready() {
  [[ "${NO_OPEN:-0}" == "1" ]] && return 0
  (
    for _ in $(seq 1 600); do
      if curl -sf -o /dev/null http://localhost:3000; then open http://localhost:3000; exit 0; fi
      sleep 2
    done
  ) &
  WATCHER=$!
  trap 'kill "$WATCHER" 2>/dev/null || true' EXIT
}

# ---------------------------------------------------------------------------
# Modos
# ---------------------------------------------------------------------------

mode_dev() {
  ensure_env; ensure_node; ensure_python; ensure_rust; ensure_db
  check_port 3000 "frontend"; check_port 8000 "API"; check_port 8001 "servicio de inteligencia"
  cat <<EOF

${BOLD}Mercastats en desarrollo${RESET}
  Aplicación    http://localhost:3000
  API           http://localhost:8000/health
  Inteligencia  http://localhost:8001/health
  Cuenta demo   (ver README)  ·  Ctrl+C para parar los 3 procesos  ·  ./run.sh stop para parar la BD

La primera vez el backend y el frontend (WebAssembly) tardan unos minutos en compilar.

EOF
  open_when_ready
  node dev.js
}

mode_docker() {
  ensure_env; ensure_docker
  say "Levantando PostgreSQL y aplicando migraciones"
  docker compose up -d --wait db
  apply_migrations
  say "Construyendo y levantando todo el stack (Ctrl+C para parar)"
  cat <<EOF
  Aplicación    http://localhost:$(env_get FRONTEND_PORT 3000)
  API           http://localhost:$(env_get BACKEND_PORT 8000)/health
  Inteligencia  http://localhost:$(env_get INTELLIGENCE_PORT 8001)/health
EOF
  exec docker compose up --build
}

mode_test() {
  ensure_env; ensure_python
  say "Tests de Python"
  (cd intelligence-service && ../.venv/bin/python -m pytest -q)
  ensure_db
  say "Tests del backend Rust (usan la base de datos de Docker)"
  local user password db port
  user="$(env_get POSTGRES_USER mercastats_app)"; password="$(env_get POSTGRES_PASSWORD '')"
  db="$(env_get POSTGRES_DB mercastats)"; port="$(env_get POSTGRES_PORT 5432)"
  DATABASE_URL="postgres://${user}:${password}@localhost:${port}/${db}" cargo test -p mercastats-backend
}

mode_stop() {
  ensure_docker
  docker compose down
  say "Base de datos detenida (volumen conservado; usa 'docker compose down --volumes' para borrarla)"
}

case "${1:-dev}" in
  dev|"")  mode_dev ;;
  docker)  mode_docker ;;
  test)    mode_test ;;
  setup)   ensure_env; ensure_node; ensure_python; ensure_rust; ensure_db; say "Entorno listo. Lanza:  ./run.sh" ;;
  stop)    mode_stop ;;
  -h|--help|help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//' ;;
  *) die "Modo desconocido: $1  (usa: dev | docker | test | setup | stop)" ;;
esac
