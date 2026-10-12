#!/usr/bin/env bash
# Funciones de instalación de Arch/CachyOS. Usa say, warn, die, have y as_root del lanzador.

install_pacman_packages() {
  (($#)) || return 0
  local log retry_cache
  log="$(mktemp "${TMPDIR:-/tmp}/mercastats-pacman.XXXXXX")"

  say "Sincronizando repositorios e instalando dependencias con pacman"
  say "Pacman mostrará la actualización del sistema y pedirá confirmación."
  if as_root env LC_ALL=C pacman -Syu --needed --color never "$@" 2>&1 | tee "$log"; then
    rm -f -- "$log"
    return 0
  fi

  # No repetir operaciones canceladas, conflictos, errores de sudo ni bloqueos de pacman.
  if ! grep -Eq '404|failed retrieving file|signature.*(invalid|unknown trust)|invalid or corrupted package|corrupted.*PGP' "$log"; then
    die "Pacman no completó la operación. Registro: $log"
  fi

  warn "Falló la descarga o la firma. Preparando un único intento de recuperación."
  if have cachyos-rate-mirrors; then
    as_root cachyos-rate-mirrors || die "No se pudieron revisar los mirrors. Registro: $log"
  fi
  if grep -Eq 'signature.*(invalid|unknown trust)|invalid or corrupted package|corrupted.*PGP' "$log"; then
    local keyrings=(archlinux)
    [[ ! -f /usr/share/pacman/keyrings/cachyos.gpg ]] || keyrings+=(cachyos)
    # Reimportar las claves distribuidas por el SO conserva el llavero existente.
    as_root pacman-key --populate "${keyrings[@]}" || die "No se pudieron reimportar las claves. Registro: $log"
  fi

  # La caché nueva impide reutilizar los archivos que fallaron; la verificación PGP sigue activa.
  retry_cache="$(mktemp -d "${TMPDIR:-/tmp}/mercastats-pacman-cache.XXXXXX")"
  chmod 755 "$retry_cache"
  if as_root env LC_ALL=C pacman -Syyu --needed --color never --cachedir "$retry_cache" "$@" 2>&1 | tee -a "$log"; then
    as_root rm -rf -- "$retry_cache" || warn "No se pudo limpiar la caché temporal: $retry_cache"
    rm -f -- "$log"
    return 0
  fi
  die "La recuperación de pacman falló. Registro: $log; caché temporal: $retry_cache"
}
