"""Pruebas del instalador sin sudo, cambios del sistema ni descargas."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
HARNESS = r'''
set -euo pipefail
say() { :; }
warn() { :; }
die() { printf '%s\n' "$*" >&2; exit 1; }
have() { [[ "$1" == cachyos-rate-mirrors ]]; }
as_root() {
  printf '%s\n' "$*" >> "$TRACE"
  if [[ "$1" != env ]]; then return 0; fi
  case "$SCENARIO:$*" in
    success:*) return 0 ;;
    cancel:*) printf 'error: transaction cancelled\n'; return 1 ;;
    missing:*'-Syu '*) printf 'error: failed retrieving file: HTTP 404\n'; return 1 ;;
    signature:*'-Syu '*) printf 'error: uv: signature from "CachyOS" is invalid\n'; return 1 ;;
    twice:*) printf 'error: invalid or corrupted package (PGP signature)\n'; return 1 ;;
    *) return 0 ;;
  esac
}
source "$INSTALLER"
if [[ "$SCENARIO" == empty ]]; then install_pacman_packages; else install_pacman_packages uv trunk rust-wasm; fi
'''


class PacmanInstallerTests(unittest.TestCase):
    def run_installer(self, scenario):
        with tempfile.TemporaryDirectory() as directory:
            trace = Path(directory) / 'commands'
            env = dict(os.environ, SCENARIO=scenario, TRACE=str(trace), TMPDIR=directory,
                       INSTALLER=str(ROOT / 'scripts/install-pacman.sh'))
            result = subprocess.run(['bash', '-c', HARNESS], env=env, capture_output=True, text=True)
            commands = trace.read_text().splitlines() if trace.exists() else []
            return result, commands

    def test_no_missing_packages_requires_no_privileged_commands(self):
        result, commands = self.run_installer('empty')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(commands, [])

    def test_install_syncs_and_upgrades_with_normal_confirmation(self):
        result, commands = self.run_installer('success')
        self.assertEqual(result.returncode, 0)
        self.assertEqual(commands, ['env LC_ALL=C pacman -Syu --needed --color never uv trunk rust-wasm'])

    def test_404_refreshes_mirrors_and_uses_fresh_cache(self):
        result, commands = self.run_installer('missing')
        self.assertEqual(result.returncode, 0)
        self.assertIn('cachyos-rate-mirrors', commands)
        retry = next(command for command in commands if ' -Syyu ' in command)
        self.assertIn('--cachedir ', retry)
        self.assertTrue(retry.endswith('uv trunk rust-wasm'))
        self.assertFalse(any('pacman-key' in command for command in commands))

    def test_bad_signature_reimports_keys_and_preserves_signature_check(self):
        result, commands = self.run_installer('signature')
        self.assertEqual(result.returncode, 0)
        self.assertTrue(any(command.startswith('pacman-key --populate archlinux') for command in commands))
        self.assertFalse(any('SigLevel' in command or '--noconfirm' in command for command in commands))

    def test_cancel_does_not_retry(self):
        result, commands = self.run_installer('cancel')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(len(commands), 1)

    def test_recovery_is_bounded_and_keeps_diagnostic_path(self):
        result, commands = self.run_installer('twice')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(sum(' LC_ALL=C pacman ' in command for command in commands), 2)
        self.assertIn('Registro:', result.stderr)
        self.assertIn('caché temporal:', result.stderr)


if __name__ == '__main__':
    unittest.main()
