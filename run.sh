#!/usr/bin/env bash
# Lanzador de Mercastats para macOS y Linux: prepara y arranca el proyecto.
#
#   ./run.sh           desarrollo: PostgreSQL en Docker + servicio Python + API Rust + frontend
#                      con recarga automática  ->  http://localhost:3000
#   ./run.sh docker    todo el stack en contenedores (docker compose up --build)
#   ./run.sh test      tests de Python y del backend Rust
#   ./run.sh setup     instala dependencias y prepara el entorno sin arrancar la aplicación
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

case "$(uname -s)" in
  Darwin) OS=macos ;;
  Linux)  OS=linux ;;
  *) die "Sistema no compatible con run.sh. En Windows nativo usa .\\run.ps1 desde PowerShell; en WSL2 usa este script." ;;
esac
say "Sistema detectado: $OS"

if [[ "$OS" == linux ]]; then
  [[ -r /etc/os-release ]] || die "No puedo identificar la distribución Linux (/etc/os-release)."
  # shellcheck disable=SC1091
  . /etc/os-release
  case " ${ID:-} ${ID_LIKE:-} " in
    *" arch "*|*" cachyos "*) PKG=pacman ;;
    *" debian "*|*" ubuntu "*) PKG=apt ;;
    *" fedora "*|*" rhel "*) PKG=dnf ;;
    *" opensuse "*|*" suse "*) PKG=zypper ;;
    *) die "Distribución Linux sin instalador configurado: ${PRETTY_NAME:-desconocida}." ;;
  esac
else
  PKG=brew
fi

as_root() {
  if (( EUID == 0 )); then "$@"; else
    have sudo || die "Hace falta sudo para instalar paquetes del sistema."
    sudo "$@"
  fi
}

# shellcheck source=scripts/install-pacman.sh
source scripts/install-pacman.sh

install_packages() {
  (($#)) || return 0
  say "Instalando paquetes del sistema: $*"
  case "$PKG" in
    brew) have brew || die "Instala Homebrew desde https://brew.sh y vuelve a ejecutar ./run.sh"; brew install "$@" ;;
    pacman) install_pacman_packages "$@" ;;
    apt) as_root apt-get update; as_root apt-get install -y "$@" ;;
    dnf) as_root dnf install -y "$@" ;;
    zypper) as_root zypper --non-interactive install "$@" ;;
  esac
}

ensure_system_tools() {
  local packages=() tool
  case "$PKG" in
    brew)
      for tool in openssl curl node pkg-config; do have "$tool" || packages+=("$tool"); done
      have python3.11 || packages+=(python@3.11)
      have tesseract || packages+=(tesseract)
      ;;
    pacman)
      for tool in openssl curl node pkg-config cc lsof; do
        have "$tool" || case "$tool" in node) packages+=(nodejs);; cc) packages+=(base-devel);; pkg-config) packages+=(pkgconf);; *) packages+=("$tool");; esac
      done
      have uv || packages+=(uv)
      have python3.11 || packages+=(python311)
      have trunk || packages+=(trunk)
      have docker || packages+=(docker docker-compose)
      if have docker && ! docker compose version >/dev/null 2>&1; then packages+=(docker-compose); fi
      have tesseract || packages+=(tesseract)
      if ! have tesseract || ! tesseract --list-langs 2>/dev/null | grep -qx spa; then
        packages+=(tesseract-data-spa)
      fi
      if have pkg-config && ! pkg-config --exists openssl; then packages+=(openssl); fi
      if [[ "$(command -v cargo 2>/dev/null || true)" == /usr/bin/cargo ]] &&
         [[ "$(command -v rustc 2>/dev/null || true)" == /usr/bin/rustc ]] &&
         pacman -Q rust >/dev/null 2>&1 && ! pacman -Q rust-wasm >/dev/null 2>&1; then
        packages+=(rust-wasm)
      elif ! pacman -Q rust >/dev/null 2>&1 && ! have rustup; then
        packages+=(rustup)
      fi
      ;;
    apt)
      for tool in openssl curl node pkg-config cc lsof; do
        have "$tool" || case "$tool" in node) packages+=(nodejs);; cc) packages+=(build-essential);; *) packages+=("$tool");; esac
      done
      have tesseract || packages+=(tesseract-ocr tesseract-ocr-spa)
      ;;
    dnf|zypper)
      for tool in openssl curl node pkg-config cc lsof; do
        have "$tool" || case "$tool" in node) packages+=(nodejs);; cc) packages+=(gcc);; *) packages+=("$tool");; esac
      done
      have tesseract || packages+=(tesseract)
      ;;
  esac
  install_packages "${packages[@]}"
  if [[ "$OS" == linux && "$PKG" != pacman ]] && ! pkg-config --exists openssl; then
    case "$PKG" in
      apt) install_packages libssl-dev ;;
      dnf) install_packages openssl-devel ;;
      zypper) install_packages libopenssl-devel ;;
    esac
  fi
  if have tesseract && ! tesseract --list-langs 2>/dev/null | grep -qx spa; then
    case "$PKG" in
      brew) install_packages tesseract-lang ;;
      pacman) die "Falta el idioma spa de Tesseract tras instalar tesseract-data-spa." ;;
      apt) install_packages tesseract-ocr-spa ;;
      dnf) install_packages tesseract-langpack-spa ;;
      zypper) warn "Instala el paquete de idioma español de Tesseract para OCR." ;;
    esac
  fi
}

sha_file() {
  if have sha256sum; then sha256sum "$1" | cut -d' ' -f1
  else shasum -a 256 "$1" | cut -d' ' -f1; fi
}

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
  have openssl || install_packages openssl
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
  if ! have docker; then
    case "$PKG" in
      brew) say "Instalando OrbStack para Docker"; brew install --cask orbstack ;;
      pacman) install_packages docker docker-compose ;;
      apt) install_packages docker.io docker-compose-v2 ;;
      dnf) install_packages moby-engine docker-compose-plugin ;;
      zypper) install_packages docker docker-compose ;;
    esac
  fi
  if ! docker compose version >/dev/null 2>&1; then
    case "$PKG" in
      pacman|zypper) install_packages docker-compose ;;
      apt) install_packages docker-compose-v2 ;;
      dnf) install_packages docker-compose-plugin ;;
      brew) die "Falta Docker Compose v2. Actualiza OrbStack o Docker Desktop." ;;
    esac
    docker compose version >/dev/null 2>&1 || die "No se pudo instalar Docker Compose v2."
  fi
  docker info >/dev/null 2>&1 && return
  say "Arrancando el motor de Docker…"
  if [[ "$OS" == macos ]]; then
    if   [[ -d /Applications/OrbStack.app ]]; then open -a OrbStack
    elif [[ -d /Applications/Docker.app   ]]; then open -a Docker
    else die "Docker no responde y no encuentro OrbStack ni Docker Desktop en /Applications."; fi
  elif have systemctl && [[ ! -f /proc/sys/fs/binfmt_misc/WSLInterop ]]; then
    as_root systemctl start docker || die "No pude arrancar Docker. Revisa: systemctl status docker"
  else
    die "Docker no responde. En WSL activa la integración de Docker Desktop; en Linux inicia el daemon de Docker."
  fi
  for _ in $(seq 1 60); do
    docker info >/dev/null 2>&1 && return
    sleep 2
  done
  die "Docker no respondió en 2 minutos. Si tu usuario no tiene acceso al socket, añade tu usuario al grupo docker y abre una sesión nueva."
}

# ¿Hay algo escuchando en este puerto del loopback? La conexión TCP detecta
# incluso procesos de otros usuarios, a diferencia de algunas consultas a lsof.
port_busy() {
  (echo >/dev/tcp/127.0.0.1/"$1") >/dev/null 2>&1 ||
    (echo >/dev/tcp/::1/"$1") >/dev/null 2>&1
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
    warn "El puerto $port ya lo usa otro servidor. Uso el $free para la BD de Mercastats."
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
  local path requirements extras stamp sum py preference
  preference=managed
  [[ "$PKG" == pacman ]] && preference=system
  for path in .venv ocr-service/.venv; do
    if [[ "$path" == .venv ]]; then
      requirements=intelligence-service/requirements.txt; extras=(pytest httpx)
    else
      requirements=ocr-service/requirements.txt; extras=()
    fi
    py="$path/bin/python"; stamp="$path/.requirements.sha"
    if [[ ! -x "$py" ]]; then
      say "Creando entorno virtual de Python ($path, Python 3.11)"
      if have uv; then
        uv venv "$path" --python 3.11 --python-preference "$preference"
      elif have python3.11; then
        python3.11 -m venv "$path"
      else
        ensure_uv
        uv venv "$path" --python 3.11 --python-preference "$preference"
      fi
    fi
    sum="$(sha_file "$requirements")"
    if [[ "$(cat "$stamp" 2>/dev/null || true)" != "$sum" ]] || ! python_deps_present "$py" "$requirements" "${extras[@]}"; then
      say "Instalando dependencias de $requirements"
      if have uv; then
        uv pip install --python "$py" -r "$requirements" "${extras[@]}"
      else
        "$py" -m pip install -r "$requirements" "${extras[@]}"
      fi
      printf '%s\n' "$sum" > "$stamp"
    fi
  done
}

python_deps_present() {
  local py="$1" requirements="$2"; shift 2
  "$py" - "$requirements" "$@" <<'PY'
import importlib.metadata as metadata
import re
import sys
from pathlib import Path

lines = Path(sys.argv[1]).read_text().splitlines() + sys.argv[2:]
for line in lines:
    line = line.split('#', 1)[0].strip()
    if not line:
        continue
    name = re.split(r'[<>=!~;\[\s]', line, 1)[0]
    try:
        metadata.version(name)
    except metadata.PackageNotFoundError:
        sys.exit(1)
PY
}

ensure_uv() {
  if [[ "$PKG" == pacman ]]; then
    install_packages uv
    have uv || die "No se pudo instalar uv con pacman."
    return
  fi
  have curl || install_packages curl
  say "Instalando uv para disponer de Python 3.11"
  local script
  script="$(mktemp)"
  curl -LsSf https://astral.sh/uv/install.sh -o "$script"
  sh "$script"
  rm -f "$script"
  export PATH="$HOME/.local/bin:$HOME/.cargo/bin:$PATH"
  have uv || die "No se pudo instalar uv."
}

ensure_rust() {
  if [[ "$PKG" == pacman ]] &&
     [[ "$(command -v cargo 2>/dev/null || true)" == /usr/bin/cargo ]] &&
     [[ "$(command -v rustc 2>/dev/null || true)" == /usr/bin/rustc ]] &&
     pacman -Q rust >/dev/null 2>&1; then
    if ! pacman -Q rust-wasm >/dev/null 2>&1; then install_packages rust-wasm; fi
  else
    if [[ "$PKG" == pacman ]] && ! have rustup && ! pacman -Q rust >/dev/null 2>&1; then
      install_packages rustup
    fi
    if ! have rustup || ! have cargo; then
      say "Instalando Rust y rustup"
      local script
      script="$(mktemp)"
      curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o "$script"
      sh "$script" -y --no-modify-path
      rm -f "$script"
      export PATH="$HOME/.cargo/bin:$PATH"
    fi
    have cargo && have rustup || die "No se pudo instalar Rust."
    if ! rustup show active-toolchain >/dev/null 2>&1; then rustup default stable; fi
    if ! rustup target list --installed | grep -q '^wasm32-unknown-unknown$'; then
      say "Añadiendo el objetivo wasm32-unknown-unknown"
      rustup target add wasm32-unknown-unknown
    fi
  fi
  if ! have trunk; then
    say "Instalando trunk (solo la primera vez)"
    case "$PKG" in
      brew|pacman) install_packages trunk ;;
      *)
        # Algunas versiones de GCC no aceptan los atributos SIMD de libdeflate-sys,
        # una dependencia de Trunk. Clang permite compilarla en esas distribuciones.
        have clang || install_packages clang
        CC=clang cargo install --locked trunk
        ;;
    esac
    have trunk || die "Trunk no quedó disponible después de instalarlo."
  fi
  local sum stamp=.cargo-fetch.sha
  sum="$(sha_file Cargo.lock)"
  if [[ "$(cat "$stamp" 2>/dev/null || true)" != "$sum" ]]; then
    say "Descargando dependencias Rust del workspace"
    cargo fetch --locked
    printf '%s\n' "$sum" > "$stamp"
  fi
}

ensure_node() {
  have node || die "No se pudo instalar Node.js (lo usa dev.js)."
}

check_port() {
  local port="$1" label="$2" pid
  port_busy "$port" || return 0
  pid="$(lsof -nP -iTCP:"$port" -sTCP:LISTEN -t 2>/dev/null | head -1 || true)"
  if [[ -n "$pid" ]]; then
    die "El puerto $port ($label) ya lo usa $(ps -p "$pid" -o comm= 2>/dev/null || echo '?') (PID $pid). Ciérralo y reintenta."
  fi
  die "El puerto $port ($label) ya está ocupado. Ciérralo y reintenta."
}

# Abre el navegador cuando el frontend responde (la primera compilación tarda minutos).
# Deja el PID del vigilante en $WATCHER para pararlo al salir.
open_when_ready() {
  [[ "${NO_OPEN:-0}" == "1" ]] && return 0
  (
    for _ in $(seq 1 600); do
      if curl -sf -o /dev/null http://localhost:3000; then
        if [[ "$OS" == macos ]]; then open http://localhost:3000
        elif have xdg-open; then xdg-open http://localhost:3000 >/dev/null 2>&1 || true
        fi
        exit 0
      fi
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
  ensure_system_tools; ensure_env; ensure_node; ensure_python; ensure_rust; ensure_db
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
  ensure_system_tools; ensure_env; ensure_python; ensure_rust
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
  setup)   ensure_system_tools; ensure_env; ensure_node; ensure_python; ensure_rust; ensure_db; say "Entorno listo. Lanza:  ./run.sh" ;;
  stop)    mode_stop ;;
  -h|--help|help) sed -n '2,13p' "$0" | sed 's/^# \{0,1\}//' ;;
  *) die "Modo desconocido: $1  (usa: dev | docker | test | setup | stop)" ;;
esac
