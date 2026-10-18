"""Real entry acceptance; all projects and repositories stay in the job tmp root."""
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]
TMP = Path(os.environ.get('UML_ENTRY_TEST_TMP', str(Path(tempfile.gettempdir()) / 'uml-entry-tests')))


class Entries(unittest.TestCase):
    def setUp(self):
        TMP.mkdir(parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(prefix='project with spaces ', dir=TMP)).resolve()
        self.project = self.root / 'sample project'
        self.project.mkdir()
        self.source = self.root / 'viewer source'
        shutil.copytree(REPO, self.source, ignore=shutil.ignore_patterns('.git', '.claude', 'target', '.cpcache', '.uml-viewer', '__pycache__'))
        subprocess.run(['git', 'init', '-b', 'test', str(self.source)], check=True, capture_output=True)
        subprocess.run(['git', '-C', str(self.source), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.source), '-c', 'user.name=Test', '-c', 'user.email=test@example.org', 'commit', '-m', 'fixture'], check=True, capture_output=True)
        self.env = dict(os.environ, UML_VIEWER_REPO_URL=str(self.source), UML_VIEWER_REF='test')
        (self.project / 'deps.edn').write_text('{:paths ["src"]}')
        (self.project / 'policy.edn').write_text('{:lang :clojure :src "src" :prefix "sample" :proposals [{:id :saved :name "Saved" :layers []}]}')
        (self.project / 'src').mkdir()
        (self.project / 'src/sample.clj').write_text('(ns sample)\n(defn greet [] "hello")\n')

    def run_entry(self, args, cwd=None, env=None):
        return subprocess.run(args, cwd=cwd or self.project, env=env or self.env, text=True, capture_output=True)

    def install(self):
        return self.run_entry(['pwsh', '-NoProfile', '-File', str(REPO / 'scripts/get-uml-viewer.ps1'), '--install-only'])

    def test_install_only_preserves_policy_and_generates_entries_on_repeat(self):
        policy = (self.project / 'policy.edn').read_bytes()
        (self.project / '.gitignore').touch()
        for _ in range(2):
            result = self.install()
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            for name in ['uml', 'uml.ps1']:
                self.assertTrue((self.project / name).is_file(), name)
            self.assertFalse((self.project / 'uml.cmd').exists())
            self.assertEqual(policy, (self.project / 'policy.edn').read_bytes())
            self.assertFalse((self.project / 'uml-viewer-log.txt').exists())
        self.assertEqual(1, (self.project / '.gitignore').read_text().count('# BEGIN UML-VIEWER'))

    def test_install_accepts_fixed_commit_without_full_clone(self):
        sha = subprocess.run(['git', '-C', str(self.source), 'rev-parse', 'HEAD'],
                             check=True, capture_output=True, text=True).stdout.strip()
        entry = self.source / 'scripts/uml.ps1'
        entry.write_text(entry.read_text() + '\n# later revision\n')
        subprocess.run(['git', '-C', str(self.source), 'add', 'scripts/uml.ps1'], check=True)
        subprocess.run(['git', '-C', str(self.source), '-c', 'user.name=Test', '-c', 'user.email=test@example.org',
                        'commit', '-m', 'later'], check=True, capture_output=True)
        self.env['UML_VIEWER_REF'] = sha
        result = self.install()
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertNotIn('later revision', (self.project / 'uml.ps1').read_text())
        commits = subprocess.run(['git', '-C', str(self.project / '.uml-viewer/uml-viewer'), 'rev-list', '--all', '--count'],
                                 check=True, capture_output=True, text=True).stdout.strip()
        self.assertEqual('1', commits)

    @unittest.skipIf(os.name == 'nt', 'Unix extensionless entry requires POSIX execution')
    def test_ir_uses_real_generator_from_another_working_directory(self):
        self.assertEqual(0, self.install().returncode)
        result = self.run_entry([str(self.project / 'uml'), 'ir', 'diagram with spaces.edn'], cwd=self.root)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        diagram = (self.project / 'diagram with spaces.edn').read_text()
        self.assertIn('sample', diagram)
        self.assertIn('Saved', diagram)

    @unittest.skipIf(os.name == 'nt', 'Unix shell tool fixture requires POSIX execution')
    def test_startup_runs_in_ps7_without_legacy_delegate_and_preserves_argv(self):
        self.assertEqual(0, self.install().returncode)
        legacy = self.project / '.uml-viewer/uml-viewer/scripts/uml-launch-unix'
        legacy.unlink(missing_ok=True)
        bin_dir = self.root / 'startup tools'
        bin_dir.mkdir()
        cli = bin_dir / 'clojure'
        cli.write_text('#!/bin/sh\ncase "$*" in *uml-viewer.adapters.companion*) printf "%s\\n" "${TEST_BACKEND:-herdr}"; exit 0;; esac\npwd > launched.txt\nprintf "%s\\n" "$@" >> launched.txt\necho fixture-stdout\necho fixture-stderr >&2\n')
        cli.chmod(0o755)
        env = dict(self.env, PATH=str(bin_dir) + os.pathsep + self.env['PATH'])
        result = self.run_entry([str(self.project / 'uml'), 'diagram with spaces.edn', 'literal;$(touch injected)', '--BACKEND', 'literal.edn'], cwd=self.root, env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        import time
        deadline = time.monotonic() + 10
        argv = []
        while time.monotonic() < deadline:
            if (self.project / 'launched.txt').exists():
                argv = (self.project / 'launched.txt').read_text().splitlines()
                if len(argv) == 12:
                    break
            time.sleep(0.05)
        self.assertEqual(str(self.project), argv[0])
        self.assertEqual(['-M', '-m', 'uml-viewer.main.uml-viewer', '--backend', 'herdr', 'diagram with spaces.edn', 'literal;$(touch injected)', '--BACKEND', 'literal.edn'], argv[3:])
        self.assertFalse((self.project / 'injected').exists())
        deadline = time.monotonic() + 10
        log = ''
        while time.monotonic() < deadline:
            log = (self.project / 'uml-viewer-log.txt').read_text()
            if 'fixture-stdout' in log and 'fixture-stderr' in log:
                break
            time.sleep(0.05)
        self.assertIn('fixture-stdout', log)
        self.assertIn('fixture-stderr', log)

    @unittest.skipIf(os.name == 'nt', 'Unix shell tool fixture requires POSIX execution')
    def test_background_start_returns_without_inheriting_captured_pipes(self):
        import time
        self.assertEqual(0, self.install().returncode)
        tools = self.root / 'detach tools'
        tools.mkdir()
        cli = tools / 'clojure'
        cli.write_text('#!/bin/sh\ncase "$*" in *uml-viewer.adapters.companion*) printf "%s\\n" "${TEST_BACKEND:-herdr}"; exit 0;; esac\nsleep 6\n')
        cli.chmod(0o755)
        env = dict(self.env, PATH=str(tools) + os.pathsep + self.env['PATH'])
        started = time.monotonic()
        result = self.run_entry([str(self.project / 'uml')], env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertLess(time.monotonic() - started, 4, 'background launcher retained captured pipes')

    @unittest.skipIf(os.name == 'nt', 'Unix shell tool fixture requires POSIX execution')
    def test_restart_waits_for_recorded_process_exit_before_launching(self):
        import json
        import time
        self.assertEqual(0, self.install().returncode)
        tools = self.root / 'restart tools'
        tools.mkdir()
        cli = tools / 'clojure'
        cli.write_text('#!/bin/sh\ncase "$*" in *uml-viewer.adapters.companion*) printf "%s\\n" "${TEST_BACKEND:-herdr}"; exit 0;; esac\ntouch restarted.txt\n')
        cli.chmod(0o755)
        env = dict(self.env, PATH=str(tools) + os.pathsep + self.env['PATH'])
        # An authoritative JVM record is required even when its process has exited.
        # This fixture verifies the live launcher wait; native restart tests cover a live JVM.
        identity = self.root / 'exited jvm identity.ps1'
        identity.write_text('$p=Get-Process -Id $PID; @{pid=$PID;started=$p.StartTime.ToUniversalTime().ToString("o")}|ConvertTo-Json|Set-Content -LiteralPath $args[0]')
        subprocess.run(['pwsh', '-NoProfile', '-File', str(identity),
                        str(self.project / '.uml-viewer/viewer-jvm.json')], check=True, capture_output=True)
        fixture = self.root / 'old viewer.ps1'
        fixture.write_text('$p=Get-Process -Id $PID; @{pid=$PID;started=$p.StartTime.ToUniversalTime().Ticks.ToString()}|ConvertTo-Json|Set-Content -LiteralPath $args[0];Start-Sleep -Seconds 4')
        old = subprocess.Popen(['pwsh', '-NoProfile', '-File', str(fixture),
                                str(self.project / '.uml-viewer/viewer-process.json')],
                               stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        record = self.project / '.uml-viewer/viewer-process.json'
        deadline = time.monotonic() + 10
        while not record.exists() and time.monotonic() < deadline:
            time.sleep(0.05)
        self.assertTrue(record.exists())
        # Reap our child as a real detached launcher's parent would.
        import threading
        reaper = threading.Thread(target=old.wait)
        reaper.start()
        result = self.run_entry([str(self.project / 'uml'), 'custom.edn', '--backend', 'herdr', '--restart'], env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIsNotNone(old.poll(), 'wrapper returned while the old viewer process was alive')
        old.communicate(timeout=10)

    @unittest.skipIf(os.name == 'nt', 'Unix shell tool fixture requires POSIX execution')
    def test_metric_alias_preserves_arguments_working_directory_and_exit(self):
        self.assertEqual(0, self.install().returncode)
        bin_dir = self.root / 'fake tools'
        bin_dir.mkdir()
        cli = bin_dir / 'clojure'
        cli.write_text('#!/bin/sh\ncase "$*" in *uml-viewer.adapters.companion*) printf "%s\\n" "${TEST_BACKEND:-herdr}"; exit 0;; esac\npwd > received.txt\nprintf "%s\\n" "$@" >> received.txt\nexit 23\n')
        cli.chmod(0o755)
        env = dict(self.env, PATH=str(bin_dir) + os.pathsep + self.env['PATH'])
        for command in ['crap', 'mutate']:
            result = self.run_entry([str(self.project / 'uml'), command, 'src/file with spaces.clj', '--mutate-all', 'literal;$value'], cwd=self.root, env=env)
            self.assertEqual(23, result.returncode, result.stderr)
            self.assertEqual([str(self.project), '-M:' + command, 'src/file with spaces.clj', '--mutate-all', 'literal;$value'], (self.project / 'received.txt').read_text().splitlines())

    @unittest.skipIf(os.name == 'nt', 'Unix shell tool fixture requires POSIX execution')
    def test_external_metrics_keep_project_cwd_and_differential_arguments(self):
        self.assertEqual(0, self.install().returncode)
        (self.project / 'deps.edn').unlink()
        for command, tool in [('crap', 'crapper'), ('mutate', 'mutator')]:
            tool_dir = self.project / '.uml-viewer' / tool
            tool_dir.mkdir()
            entry = tool_dir / tool
            entry.write_text('#!/bin/sh\npwd > external.txt\nprintf "%s\\n" "$@" >> external.txt\nexit 7\n')
            entry.chmod(0o755)
            result = self.run_entry([str(self.project / 'uml'), command, 'src/file with spaces.lua', 'literal;$value'], cwd=self.root)
            self.assertEqual(7, result.returncode, result.stderr)
            forwarded = (['--full-tree'] if command == 'crap' else []) + ['src/file with spaces.lua', 'literal;$value']
            self.assertEqual([str(self.project), *forwarded], (self.project / 'external.txt').read_text().splitlines())

    @unittest.skipIf(os.name == 'nt', 'Unix extensionless entry requires POSIX execution')
    def test_unix_installer_forwards_install_only_and_rejects_invalid_arguments(self):
        entry = str(REPO / 'scripts/get-uml-viewer')
        result = self.run_entry([entry, '--install-only'])
        self.assertEqual(0, result.returncode, result.stderr)
        result = self.run_entry([entry, '--install-only', 'extra'])
        self.assertNotEqual(0, result.returncode)
        self.assertIn('usage:', result.stderr)

    @unittest.skipIf(os.name == 'nt', 'Unix extensionless entry and PATH diagnostics require POSIX execution')
    def test_missing_dependencies_and_policy_are_diagnostic(self):
        self.assertEqual(0, self.install().returncode)
        (self.project / 'policy.edn').unlink()
        result = self.run_entry([str(self.project / 'uml'), 'ir'])
        self.assertNotEqual(0, result.returncode)
        self.assertIn('no policy', result.stderr)
        (self.project / 'deps.edn').unlink()
        result = self.run_entry([str(self.project / 'uml'), 'mutate', 'file.lua'])
        self.assertNotEqual(0, result.returncode)
        self.assertIn('MUTATOR_REPO_URL', result.stderr)
        env = dict(self.env, PATH=str(self.root / 'empty path'))
        result = self.run_entry([str(self.project / 'uml'), 'crap'], env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('PowerShell 7', result.stderr)
        self.assertIn('https://aka.ms/powershell-install', result.stderr)
        pwsh = shutil.which('pwsh')
        (self.project / 'deps.edn').write_text('{}')
        result = self.run_entry([pwsh, '-NoProfile', '-File', str(self.project / 'uml.ps1'), 'crap'], env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('Clojure CLI', result.stderr)
        result = self.run_entry([pwsh, '-NoProfile', '-File', str(REPO / 'scripts/get-uml-viewer.ps1'), '--install-only'], env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('Install Git', result.stderr)

    @unittest.skipIf(os.name == 'nt', 'Unix standalone shell installer requires POSIX execution')
    def test_standalone_unix_bootstrap_uses_same_installer(self):
        standalone = self.root / 'downloaded installer'
        shutil.copyfile(REPO / 'scripts/get-uml-viewer', standalone)
        standalone.chmod(0o755)
        env = dict(self.env, UML_VIEWER_INSTALLER_URL=(REPO / 'scripts/get-uml-viewer.ps1').as_uri())
        result = self.run_entry([str(standalone), '--install-only'], env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertTrue((self.project / 'uml.ps1').exists())

    def test_install_without_deps_fetches_metric_checkouts_without_creating_policy(self):
        (self.project / 'deps.edn').unlink()
        (self.project / 'policy.edn').unlink()
        self.env.update(CRAPPER_REPO_URL=str(self.source), CRAPPER_REF='test', MUTATOR_REPO_URL=str(self.source), MUTATOR_REF='test')
        self.assertEqual(0, self.install().returncode)
        for tool in ['crapper', 'mutator']:
            self.assertTrue((self.project / '.uml-viewer' / tool / '.git').exists())
        self.assertFalse((self.project / 'policy.edn').exists())
        self.assertFalse((self.project / 'examples').exists())

    def test_background_launcher_invokes_powershell_cli_as_data(self):
        import json
        cli = self.root / 'clojure with spaces.ps1'
        cli.write_text('[IO.File]::WriteAllText((Join-Path (Get-Location) "script-argv.txt"), (ConvertTo-Json -InputObject @($args) -Compress))\nexit 17\n')
        manifest = self.root / 'launch.json'
        manifest.write_text(json.dumps({'executable': str(cli), 'cwd': str(self.project),
                                        'log': str(self.project / 'launch.log'),
                                        'argv': ['with spaces', 'literal;$(throw 1)', 'quoted "value"', '', 'first\nsecond'], 'powershell': True}))
        result = self.run_entry(['pwsh', '-NoProfile', '-File', str(REPO / 'scripts/uml-launch.ps1'), str(manifest)])
        self.assertEqual(17, result.returncode, result.stderr)
        self.assertEqual(['with spaces', 'literal;$(throw 1)', 'quoted "value"', '', 'first\nsecond'], json.loads((self.project / 'script-argv.txt').read_text()))

    def test_windows_clojure_batch_fails_before_background_launch_with_guidance(self):
        self.assertEqual(0, self.install().returncode)
        tools = self.root / 'batch tools'
        tools.mkdir()
        runner = self.root / 'batch boundary.ps1'
        runner.write_text("Set-Variable IsWindows -Value $true -Force\n"
                          "function Get-Command { param($Name) if($Name -eq 'clojure') { "
                          "[pscustomobject]@{CommandType='Application';Source='C:/tools/clojure.cmd';ModuleName='';Name='clojure.cmd'} "
                          "} else { Microsoft.PowerShell.Core\\Get-Command $Name } }\n"
                          "& $env:TEST_ENTRY @args;exit $LASTEXITCODE\n")
        env = dict(self.env, TEST_ENTRY=str(self.project / 'uml.ps1'), TEST_BACKEND='herdr', PATH=str(tools) + os.pathsep + self.env['PATH'])
        result = self.run_entry(['pwsh', '-NoProfile', '-File', str(runner)], env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('ClojureTools', result.stderr)
        self.assertIn('.cmd/.bat', result.stderr)
        self.assertFalse((self.project / '.uml-viewer/viewer-process.json').exists())

    def test_backend_preflight_real_installed_entry_has_no_launch_side_effects(self):
        import json
        import time
        self.assertEqual(0, self.install().returncode)
        native_entry = (['pwsh', '-NoProfile', '-File', str(self.project / 'uml.ps1')]
                        if os.name == 'nt' else [str(self.project / 'uml')])
        help_result = self.run_entry(native_entry + ['--help'])
        self.assertEqual(0, help_result.returncode)
        self.assertEqual('usage: uml [--runtime jvm|love] [--backend herdr] [--restart | ir | crap | mutate] [args]',
                         help_result.stdout.strip())
        tools = self.root / 'backend tools'
        tools.mkdir()
        herdr = tools / ('herdr.ps1' if os.name == 'nt' else 'herdr')
        herdr.write_text('Write-Output "herdr 0.9.4"\n' if os.name == 'nt'
                         else '#!/bin/sh\necho herdr 0.9.4\n')
        herdr.chmod(0o755)
        # JVM ProcessBuilder cannot invoke a PS1 shim. Exclude real Herdr binaries
        # so Windows preflight safely refuses the missing executable/version.
        tool_path = self.env['PATH']
        if os.name == 'nt':
            tool_path = os.pathsep.join(part for part in tool_path.split(os.pathsep)
                                       if not (Path(part.strip('"')) / 'herdr.exe').is_file())
        env = dict(self.env, PATH=str(tools) + os.pathsep + tool_path)
        boundary = self.root / 'windows backend boundary.ps1'
        boundary.write_text('Set-Variable IsWindows -Value $true -Force\n& $env:TEST_ENTRY @args;exit $LASTEXITCODE\n')
        env['TEST_ENTRY'] = str(self.project / 'uml.ps1')
        elapsed = []
        windows_entry = ['pwsh', '-NoProfile', '-File', str(boundary)]
        entries = [native_entry] if os.name == 'nt' else [native_entry, windows_entry]
        for entry in entries:
            unsupported = entry == native_entry and sys.platform.startswith('linux')
            for args, diagnostic in [(['--backend', 'invalid'], 'Unknown or duplicate'),
                                     (['--backend'], 'Unknown or duplicate'),
                                     (['--backend', 'tmux'], 'companions are retired'),
                                     (['--backend', 'psmux'], 'companions are retired'),
                                     (['--backend', 'herdr'], 'Linux/WSL companion launch is unsupported' if unsupported else 'Herdr 0.9.3')]:
                with self.subTest(entry=entry, args=args):
                    started = time.monotonic()
                    result = self.run_entry(entry + args, env=env)
                    elapsed.append(time.monotonic() - started)
                    self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                    self.assertIn(diagnostic, result.stderr)
                    self.assertEqual('', result.stdout)
                    self.assertFalse((self.project / '.uml-viewer/viewer-process.json').exists())
                    self.assertFalse(list((self.project / '.uml-viewer').glob('launch-*.json')))
                    self.assertFalse((self.project / 'uml-viewer-log.txt').exists())
        # A missing executable is checked at the supported Windows process boundary.
        herdr.write_text('Write-Error "herdr: command not found"; exit 127\n' if os.name == 'nt'
                         else '#!/bin/sh\necho "herdr: command not found" >&2\nexit 127\n')
        supported_entry = native_entry if os.name == 'nt' else windows_entry
        result = self.run_entry(supported_entry + ['--backend', 'herdr'], env=env)
        self.assertEqual(1, result.returncode)
        self.assertIn('Herdr 0.9.3', result.stderr)
        self.assertFalse((self.project / 'uml-viewer-log.txt').exists())
        # Legacy ownership is refused before any backend command on both boundaries.
        marker_body = ('[IO.File]::WriteAllText("backend-called.txt", "called"); exit 99\n'
                       if os.name == 'nt' else '#!/bin/sh\ntouch backend-called.txt\nexit 99\n')
        herdr.write_text(marker_body)
        for name in ['tmux', 'psmux']:
            tool = tools / (name + '.ps1' if os.name == 'nt' else name)
            tool.write_text(marker_body)
            tool.chmod(0o755)
        record = self.project / '.uml-viewer/companion.edn'
        queues = self.project / '.uml-viewer/to-agent.edn'
        queues.write_text('{:next-id 2 :queue [{:id 1 :op :display}]}')
        session = self.project / '.uml-viewer/session.edn'
        session.write_text('{:diagram-path "saved.edn"}')
        for backend in [':tmux', ':psmux', None]:
            for entry in entries:
                with self.subTest(backend=backend, entry=entry):
                    record.write_text('{' + (':backend ' + backend + ' ' if backend else '') +
                                      ':session "saved" :pane "%1" :cwd ' + json.dumps(str(self.project)) + '}')
                    originals = {path: path.read_bytes() for path in [record, queues, session]}
                    result = self.run_entry(entry + ['--restart', '--backend', 'herdr'], env=env)
                    self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                    self.assertIn('companions are retired', result.stderr)
                    self.assertIn('Do not use --restart to migrate', result.stderr)
                    for path, original in originals.items():
                        self.assertEqual(original, path.read_bytes())
                    self.assertFalse((self.project / 'backend-called.txt').exists())
                    self.assertFalse((self.project / '.uml-viewer/viewer-process.json').exists())
                    self.assertFalse(list((self.project / '.uml-viewer').glob('launch-*.json')))
                    self.assertFalse((self.project / 'uml-viewer-log.txt').exists())
        print('real backend preflight seconds: ' + ', '.join(f'{value:.2f}' for value in elapsed))

    @unittest.skipIf(os.name == 'nt', 'Unix shell verifier fixture requires POSIX execution')
    def test_empty_or_failed_foreground_verifier_cannot_launch(self):
        self.assertEqual(0, self.install().returncode)
        tools = self.root / 'verifier tools'
        tools.mkdir()
        cli = tools / 'clojure'
        cli.chmod(0o755) if cli.exists() else None
        env = dict(self.env, PATH=str(tools) + os.pathsep + self.env['PATH'])
        for body, code, diagnostic in [('exit 0', 1, 'invalid output'),
                                       ('echo verifier-failed >&2; exit 23', 23, 'verifier-failed')]:
            cli.write_text('#!/bin/sh\n' + body + '\n')
            cli.chmod(0o755)
            result = self.run_entry([str(self.project / 'uml')], env=env)
            self.assertEqual(code, result.returncode)
            self.assertIn(diagnostic, result.stderr)
            self.assertFalse(list((self.project / '.uml-viewer').glob('launch-*.json')))
            self.assertFalse((self.project / 'uml-viewer-log.txt').exists())

    def test_windows_metric_process_boundary_uses_preinstalled_python_module(self):
        # Simulate the OS boundary only; invoke the generated PS1 and real fixture process.
        self.assertEqual(0, self.install().returncode)
        (self.project / 'deps.edn').unlink()
        runner = self.root / 'windows boundary.ps1'
        runner.write_text('Set-Variable IsWindows -Value $true -Force\n& $env:TEST_ENTRY @args\nexit $LASTEXITCODE\n')
        env = dict(self.env, TEST_ENTRY=str(self.project / 'uml.ps1'))
        result = self.run_entry(['pwsh', '-NoProfile', '-File', str(runner), 'mutate'], env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('py -3 -m venv', result.stderr)
        self.assertIn('pip install -e', result.stderr)
        for command, tool in [('crap', 'crapper'), ('mutate', 'mutator')]:
            venv = self.project / '.uml-viewer' / tool / '.venv'
            if os.name == 'nt':
                subprocess.run([sys.executable, '-I', '-m', 'venv', '--without-pip', str(venv)],
                               check=True, capture_output=True)
                package = self.project / tool
                package.mkdir()
                (package / '__init__.py').write_text('')
                (package / '__main__.py').write_text(
                    'import os, pathlib, sys\n'
                    'pathlib.Path("windows.txt").write_text("\\n".join([os.getcwd(), *sys.orig_argv[1:]]))\n'
                    'print("native fixture stdout")\n'
                    'print("native fixture stderr", file=sys.stderr)\n'
                    'sys.exit(19)\n')
            else:
                directory = venv / 'Scripts'
                directory.mkdir(parents=True)
                python = directory / 'python.exe'
                python.write_text('#!/bin/sh\npwd > windows.txt\nprintf "%s\\n" "$@" >> windows.txt\necho native-fixture-stdout\necho native-fixture-stderr >&2\nexit 19\n')
                python.chmod(0o755)
            result = self.run_entry(['pwsh', '-NoProfile', '-File', str(runner), command, 'src/file with spaces.lua'], cwd=self.root, env=env)
            self.assertEqual(19, result.returncode, result.stderr)
            forwarded = (['--full-tree'] if command == 'crap' else []) + ['src/file with spaces.lua']
            self.assertEqual([str(self.project), '-m', tool, *forwarded], (self.project / 'windows.txt').read_text().splitlines())
            self.assertIn('stdout', result.stdout)
            self.assertIn('stderr', result.stderr)


if __name__ == '__main__':
    unittest.main()
