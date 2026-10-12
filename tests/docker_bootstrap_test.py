"""Arranque de Docker con comandos simulados; no usa sudo ni el servicio real."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[1]
HARNESS = r'''
set -euo pipefail
OS="${TEST_OS:-linux}"
PKG=pacman
say() { :; }
warn() { printf '%s\n' "$*" >&2; }
die() { printf '%s\n' "$*" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }
uname() { printf '%s\n' "$KERNEL"; }
sleep() { :; }
systemctl() { :; }
install_packages() { die 'Unexpected package install'; }
docker() {
  printf 'docker %s\n' "$*" >> "$TRACE"
  if [[ "$1" == --host ]]; then shift 2; fi
  case "$1" in
    context) printf '%s\n' "$ENDPOINT" ;;
    compose|inspect) return 0 ;;
    info)
      case "$SCENARIO" in
        running) return 0 ;;
        permission|remote|cancel)
          [[ "${ELEVATED:-0}" == 1 ]] && return 0
          printf 'permission denied while trying to connect to Docker\n' >&2 ;;
        *)
          if [[ -f "$READY" ]]; then
            [[ "$SCENARIO" != after_start || "${ELEVATED:-0}" == 1 ]] && return 0
            printf 'permission denied while trying to connect to Docker\n' >&2
          else
            printf 'Cannot connect to the Docker daemon\n' >&2
          fi ;;
      esac
      return 1 ;;
  esac
}
sudo() {
  printf 'sudo %s\n' "$*" >> "$TRACE"
  if [[ "$1" == -v ]]; then [[ "$SCENARIO" != cancel ]]; return; fi
  shift # docker
  ELEVATED=1 docker "$@"
}
as_root() {
  printf 'root %s\n' "$*" >> "$TRACE"
  case "$*" in
    'systemctl start docker.service')
      [[ "$SCENARIO" != failed ]] || return 1
      touch "$READY" ;;
    journalctl*) printf 'Docker journal diagnostic\n' ;;
  esac
}
source "$BOOTSTRAP"
if [[ "$ACTION" == kernel ]]; then
  check_docker_kernel "$MODULES"
elif [[ "$ACTION" == endpoint ]]; then
  docker_endpoint
else
  # Los casos del kernel se prueban aparte con árboles temporales de módulos.
  check_docker_kernel() {
    printf 'kernel check\n' >> "$TRACE"
    [[ "$SCENARIO" != kernel ]] || die 'Reinicia el sistema'
  }
  ensure_docker
  docker_cmd inspect mercastats_db
  docker_cmd compose up -d db
fi
'''


class DockerBootstrapTests(unittest.TestCase):
    def run_bootstrap(self, scenario='stopped', action='bootstrap', kernels=('test-kernel',), **extra):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            modules = path / 'modules'
            modules.mkdir()
            for kernel in kernels:
                (modules / kernel).mkdir()
            trace = path / 'commands'
            env = dict(os.environ, SCENARIO=scenario, ACTION=action, MODULES=str(modules),
                       TRACE=str(trace), READY=str(path / 'ready'), KERNEL='test-kernel',
                       ENDPOINT='unix:///var/run/docker.sock',
                       BOOTSTRAP=str(ROOT / 'scripts/ensure-docker.sh'))
            env.pop('DOCKER_HOST', None)
            env.pop('DOCKER_CONTEXT', None)
            env.update(extra)
            result = subprocess.run(['bash', '-c', HARNESS], env=env, capture_output=True, text=True)
            commands = trace.read_text().splitlines() if trace.exists() else []
            return result, commands

    def test_replaced_kernel_stops_before_any_privileged_command(self):
        result, commands = self.run_bootstrap(action='kernel', kernels=('new-kernel',))
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('test-kernel', result.stderr)
        self.assertIn('new-kernel', result.stderr)
        self.assertIn('reinicia el sistema', result.stderr)
        self.assertEqual(commands, [])
        result, commands = self.run_bootstrap(scenario='kernel')
        self.assertNotEqual(result.returncode, 0)
        self.assertFalse(any(command.startswith(('sudo ', 'root ')) for command in commands))

    def test_matching_kernel_and_wsl_do_not_require_reboot(self):
        for extra in ({}, {'KERNEL': 'microsoft-standard-WSL2'}, {'TEST_OS': 'macos'}):
            with self.subTest(extra=extra):
                result, _ = self.run_bootstrap(action='kernel', **extra)
                self.assertEqual(result.returncode, 0, result.stderr)

    def test_running_docker_requires_no_service_or_sudo_commands(self):
        result, commands = self.run_bootstrap(scenario='running')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertFalse(any(command.startswith(('sudo ', 'root ')) for command in commands))

    def test_permission_denied_uses_same_local_host_for_all_docker_commands(self):
        result, commands = self.run_bootstrap(scenario='permission')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('sudo -v', commands)
        self.assertIn('sudo docker --host unix:///var/run/docker.sock inspect mercastats_db', commands)
        self.assertIn('sudo docker --host unix:///var/run/docker.sock compose up -d db', commands)
        self.assertFalse(any(command.startswith('root ') for command in commands))

    def test_stopped_service_resets_failure_counter_and_starts(self):
        result, commands = self.run_bootstrap()
        self.assertEqual(result.returncode, 0, result.stderr)
        reset = commands.index('root systemctl reset-failed docker.service')
        start = commands.index('root systemctl start docker.service')
        self.assertLess(reset, start)

    def test_new_socket_without_access_is_handled_after_start(self):
        result, commands = self.run_bootstrap(scenario='after_start')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('root systemctl start docker.service', commands)
        self.assertIn('sudo docker --host unix:///var/run/docker.sock inspect mercastats_db', commands)

    def test_service_failure_prints_journal_and_stops(self):
        result, commands = self.run_bootstrap(scenario='failed')
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('Docker journal diagnostic', result.stderr)
        self.assertEqual(commands.count('root systemctl start docker.service'), 1)
        self.assertFalse(any('compose up' in command for command in commands))

    def test_remote_and_rootless_failures_do_not_start_system_daemon(self):
        for endpoint in ('ssh://remote', 'unix:///run/user/1000/docker.sock'):
            with self.subTest(endpoint=endpoint):
                result, commands = self.run_bootstrap(scenario='remote', ENDPOINT=endpoint)
                self.assertNotEqual(result.returncode, 0)
                self.assertFalse(any(command.startswith(('sudo ', 'root ')) for command in commands))

    def test_sudo_cancellation_does_not_retry_or_start_service(self):
        result, commands = self.run_bootstrap(scenario='cancel')
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(commands.count('sudo -v'), 1)
        self.assertFalse(any(command.startswith('root ') for command in commands))

    def test_environment_context_takes_priority_over_host(self):
        result, commands = self.run_bootstrap(action='endpoint', DOCKER_HOST='tcp://other:2375',
                                              DOCKER_CONTEXT='remote', ENDPOINT='ssh://remote')
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), 'ssh://remote')
        self.assertTrue(any('context inspect remote' in command for command in commands))
        result, commands = self.run_bootstrap(action='endpoint', DOCKER_HOST='tcp://other:2375')
        self.assertEqual(result.stdout.strip(), 'tcp://other:2375')
        self.assertEqual(commands, [])


if __name__ == '__main__':
    unittest.main()
