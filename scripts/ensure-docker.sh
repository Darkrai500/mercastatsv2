#!/usr/bin/env bash
# Funciones del lanzador; se usan también en las pruebas sin tocar el sistema.

DOCKER_CMD=(docker)
docker_cmd() { "${DOCKER_CMD[@]}" "$@"; }

docker_endpoint() {
  if [[ -n "${DOCKER_CONTEXT:-}" ]]; then
    docker context inspect "$DOCKER_CONTEXT" --format '{{.Endpoints.docker.Host}}'
  elif [[ -n "${DOCKER_HOST:-}" ]]; then
    printf '%s\n' "$DOCKER_HOST"
  else
    docker context inspect --format '{{.Endpoints.docker.Host}}'
  fi
}

check_docker_kernel() {
  local modules="${1:-/usr/lib/modules}" running dir installed=""
  [[ "$OS" == linux ]] || return 0
  running="$(uname -r)"
  # WSL administra su kernel fuera de la distribución.
  case "$running" in *[Mm]icrosoft*|*[Ww][Ss][Ll]*) return 0 ;; esac
  [[ ! -f /proc/sys/fs/binfmt_misc/WSLInterop ]] || return 0
  [[ -d "$modules" && ! -d "$modules/$running" ]] || return 0
  for dir in "$modules"/*; do
    [[ ! -d "$dir" ]] || installed+=" ${dir##*/}"
  done
  warn "Kernel en uso: $running. Módulos instalados:${installed:- ninguno}."
  die "La actualización ha sustituido los módulos del kernel en uso. Docker necesita esos módulos para la red NAT. Guarda tu trabajo, reinicia el sistema y vuelve a ejecutar ./run.sh."
}

docker_use_sudo_if_needed() {
  local endpoint="$1" error="$2"
  [[ "$OS" == linux && "${DOCKER_CMD[0]}" == docker ]] || return 0
  case "$endpoint" in unix:///var/run/docker.sock|unix:///run/docker.sock) ;; *) return 0 ;; esac
  [[ "$error" == *"permission denied"* ]] || return 0
  have sudo || die "Falta acceso al socket local de Docker y no hay sudo disponible."
  warn "Tu sesión no tiene acceso al socket local de Docker. Usaré sudo solo para los comandos de Docker."
  sudo -v || die "No se pudo autorizar el acceso a Docker con sudo."
  # El host explícito conserva el daemon elegido, aunque root use otro contexto.
  DOCKER_CMD=(sudo docker --host "$endpoint")
}

ensure_docker() {
  local endpoint error
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
  if error="$(LC_ALL=C docker_cmd info 2>&1)"; then return 0; fi
  endpoint="$(docker_endpoint)" || die "No puedo identificar el contexto de Docker. $error"
  if [[ "$OS" == linux ]]; then
    case "$endpoint" in
      unix:///var/run/docker.sock|unix:///run/docker.sock) check_docker_kernel ;;
      *) die "Docker no responde en el contexto seleccionado. $error" ;;
    esac
    docker_use_sudo_if_needed "$endpoint" "$error"
    docker_cmd info >/dev/null 2>&1 && return 0
  fi
  say "Arrancando el motor de Docker…"
  if [[ "$OS" == macos ]]; then
    if   [[ -d /Applications/OrbStack.app ]]; then open -a OrbStack
    elif [[ -d /Applications/Docker.app   ]]; then open -a Docker
    else die "Docker no responde y no encuentro OrbStack ni Docker Desktop en /Applications."; fi
  elif have systemctl && [[ ! -f /proc/sys/fs/binfmt_misc/WSLInterop ]] &&
       [[ "$(uname -r)" != *[Mm]icrosoft* && "$(uname -r)" != *[Ww][Ss][Ll]* ]]; then
    # Permite reintentar después de corregir una avería que agotó el límite de arranques.
    as_root systemctl reset-failed docker.service || die "No pude restablecer el estado del servicio de Docker."
    if ! as_root systemctl start docker.service; then
      warn "Docker no pudo arrancar. Últimas entradas del servicio:"
      as_root journalctl -u docker.service -b -n 35 --no-pager >&2 || true
      die "No pude arrancar Docker; el motivo aparece en el registro anterior."
    fi
  else
    die "Docker no responde. En WSL activa la integración de Docker Desktop; en Linux inicia el daemon de Docker."
  fi
  for _ in $(seq 1 60); do
    if error="$(LC_ALL=C docker_cmd info 2>&1)"; then return 0; fi
    docker_use_sudo_if_needed "$endpoint" "$error"
    sleep 2
  done
  die "Docker no respondió en 2 minutos. Último error: $error"
}
