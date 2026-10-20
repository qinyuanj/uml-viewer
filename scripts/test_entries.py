"""Installer and public-entry acceptance for the LOVE-only cutover.

All projects, repositories and tool fixtures stay in a job temp root. The suite
never launches the real viewer or touches an examined project's companion
resources.
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time
import unittest

REPO = Path(__file__).resolve().parents[1]
TMP = Path(os.environ.get('UML_ENTRY_TEST_TMP', str(Path(tempfile.gettempdir()) / 'uml-entry-tests')))
NO_WINDOW = getattr(subprocess, 'CREATE_NO_WINDOW', 0)
# Fixture venvs are created by the interpreter that runs this suite. The
# WindowsApps sys.executable is a protected reparse stub, so invoke it by its
# PATH name instead of copying the file.
REAL_PYTHON = 'python' if os.name == 'nt' else sys.executable


class Entries(unittest.TestCase):
    def setUp(self):
        TMP.mkdir(parents=True, exist_ok=True)
        self.root = Path(tempfile.mkdtemp(prefix='project with spaces ', dir=TMP)).resolve()
        self.project = self.root / 'sample project'
        self.project.mkdir()
        self.source = self.root / 'viewer source'
        shutil.copytree(REPO, self.source, ignore=shutil.ignore_patterns(
            '.git', '.claude', 'target', '.cpcache', '.uml-viewer', '__pycache__'))
        subprocess.run(['git', 'init', '-b', 'test', str(self.source)], check=True, capture_output=True)
        subprocess.run(['git', '-C', str(self.source), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(self.source), '-c', 'user.name=Test', '-c', 'user.email=test@example.org',
                        'commit', '-m', 'fixture'], check=True, capture_output=True)
        # Tool prerequisite fixtures.
        self.tools = self.root / 'prerequisite tools'
        self.tools.mkdir()
        (self.tools / 'love.ps1').write_text(
            "if ($args -contains '--version') { Write-Output 'LOVE 11.5 (Mysterious Mysteries)'; exit 0 }\n"
            "if ($args -contains 'companion-preflight') { Write-Output 'herdr'; exit 0 }\n"
            "$args | ConvertTo-Json -AsArray | Set-Content -LiteralPath $env:UML72_RECORD\n")
        (self.tools / 'herdr.ps1').write_text(
            "if ($args -contains '--version') { Write-Output 'herdr 0.9.5'; exit 0 }\n"
            "Write-Output '{\"$schema\":\"fixture\",\"protocol\":22,\"schema_version\":1}'; exit 0\n")
        self.python_stub = self.tools / 'python stub.ps1'
        self.pip_log = self.root / 'pip.log'
        self.python_stub.write_text(
            "if ($args[0] -eq '-c') { Write-Output '311'; exit 0 }\n"
            "if ($args[0] -eq '-m' -and $args[1] -eq 'venv') {\n"
            "  $dir=$args[2]; & $env:UML72_REAL_PYTHON -m venv $dir; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n"
            "  New-Item -ItemType Directory -Force \"$dir/Lib/site-packages/pip\" | Out-Null\n"
            "  Set-Content -LiteralPath \"$dir/Lib/site-packages/pip/__main__.py\" -Force -Value ('import os, sys' + [Environment]::NewLine + \"open(os.environ['UML72_PIP_LOG'],'a').write(' '.join(sys.argv[1:])+chr(10))\" + [Environment]::NewLine + 'raise SystemExit(0)')\n"
            "  exit 0 }\n"
            "exit 0\n")
        self.cimgui = self.root / 'cimgui'
        (self.cimgui / 'cimgui').mkdir(parents=True)
        (self.cimgui / 'cimgui.dll').write_bytes(b'fixture')
        (self.cimgui / 'cimgui/init.lua').write_text('return {}')
        self.record = self.root / 'love-received.json'
        self.env = dict(os.environ,
                        UML_VIEWER_REPO_URL=str(self.source), UML_VIEWER_REF='test',
                        CRAPPER_REPO_URL=str(self.source), CRAPPER_REF='test',
                        MUTATOR_REPO_URL=str(self.source), MUTATOR_REF='test',
                        UML_LOVE_EXECUTABLE=str(self.tools / 'love.ps1'),
                        UML_CIMGUI_DIRECTORY=str(self.cimgui),
                        UML_PYTHON_EXECUTABLE=str(self.python_stub),
                        UML72_REAL_PYTHON=str(REAL_PYTHON),
                        UML72_PIP_LOG=str(self.pip_log),
                        UML72_RECORD=str(self.record),
                        PATH=str(self.tools) + os.pathsep + os.environ.get('PATH', ''))
        self.env.pop('UML_VIEWER_TOOL_ROOT', None)
        (self.project / 'deps.edn').write_text('{:paths ["src"]}')
        (self.project / 'policy.edn').write_text('{:lang :lua :src "src" :prefix "sample" :proposals [{:id :saved :name "Saved" :layers []}]}')
        (self.project / 'src').mkdir()
        (self.project / 'src/sample.lua').write_text('local M = {}\nfunction M.greet() return "hello" end\nreturn M\n')

    def run_entry(self, args, cwd=None, env=None):
        return subprocess.run(args, cwd=cwd or self.project, env=env or self.env, text=True,
                              capture_output=True, creationflags=NO_WINDOW)

    def install(self, env=None):
        return self.run_entry(['pwsh', '-NoProfile', '-File', str(REPO / 'scripts/get-uml-viewer.ps1'),
                               '--install-only'], env=env)

    def entry(self):
        return ['pwsh', '-NoProfile', '-File', str(self.project / 'uml.ps1')]

    def test_install_only_fetches_both_tools_and_provisions_venvs_without_policy_writes(self):
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
        # Both tools are fetched even though deps.edn remains from the removed
        # Clojure implementation, and the dispatcher's project-local venvs exist.
        for tool in ['crapper', 'mutator']:
            self.assertTrue((self.project / '.uml-viewer' / tool / '.git').exists(), tool)
            self.assertTrue((self.project / '.uml-viewer' / tool / '.venv/Scripts/python.exe').exists(), tool)
        pip = self.pip_log.read_text()
        self.assertIn('install --disable-pip-version-check -e .uml-viewer/crapper', pip)
        self.assertIn('install --disable-pip-version-check -e .uml-viewer/mutator', pip)
        self.assertFalse((self.project / 'examples').exists())

    def test_install_defaults_to_main_for_viewer_and_both_tools(self):
        subprocess.run(['git', '-C', str(self.source), 'branch', '-m', 'main'], check=True)
        env = self.env.copy()
        for name in ['UML_VIEWER_REF', 'CRAPPER_REF', 'MUTATOR_REF']:
            env.pop(name, None)
        result = self.install(env=env)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        expected = subprocess.check_output(
            ['git', '-C', str(self.source), 'rev-parse', 'main'], text=True).strip()
        for tool in ['uml-viewer', 'crapper', 'mutator']:
            actual = subprocess.check_output(
                ['git', '-C', str(self.project / '.uml-viewer' / tool), 'rev-parse', 'HEAD'],
                text=True).strip()
            self.assertEqual(expected, actual, tool)

    def test_install_accepts_fixed_commit_without_full_clone(self):
        sha = subprocess.run(['git', '-C', str(self.source), 'rev-parse', 'HEAD'],
                             check=True, capture_output=True, text=True).stdout.strip()
        entry = self.source / 'scripts/uml.ps1'
        entry.write_text(entry.read_text() + '\n# later revision\n')
        subprocess.run(['git', '-C', str(self.source), 'add', 'scripts/uml.ps1'], check=True)
        subprocess.run(['git', '-C', str(self.source), '-c', 'user.name=Test', '-c', 'user.email=test@example.org',
                        'commit', '-m', 'later'], check=True, capture_output=True)
        env = dict(self.env, UML_VIEWER_REF=sha)
        result = self.install(env=env)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertNotIn('later revision', (self.project / 'uml.ps1').read_text())
        commits = subprocess.run(['git', '-C', str(self.project / '.uml-viewer/uml-viewer'), 'rev-list', '--all', '--count'],
                                 check=True, capture_output=True, text=True).stdout.strip()
        self.assertEqual('1', commits)

    def test_installer_reports_each_missing_prerequisite(self):
        cases = [
            ('LOVE 11.5 is required', dict(self.env, UML_LOVE_EXECUTABLE=str(self.root / 'missing-love.exe'))),
            ('cimgui', dict(self.env, UML_CIMGUI_DIRECTORY=str(self.root / 'empty cimgui'))),
            ('Herdr >= 0.9.3', dict(self.env, PATH=str(self.root / 'no herdr tools'))),
            ('Python 3.11+', dict(self.env, UML_PYTHON_EXECUTABLE=str(self.root / 'missing-python.exe'))),
        ]
        (self.root / 'empty cimgui').mkdir()
        no_herdr = self.root / 'no herdr tools'
        no_herdr.mkdir()
        # Only git remains discoverable so the prerequisite check is reached.
        (no_herdr / 'git.ps1').write_text('exit 0\n')
        for diagnostic, env in cases:
            with self.subTest(diagnostic=diagnostic):
                result = self.install(env=env)
                self.assertNotEqual(0, result.returncode)
                self.assertIn(diagnostic, result.stderr)

    def test_help_and_default_love_dispatch(self):
        self.assertEqual(0, self.install().returncode)
        result = self.run_entry(self.entry() + ['--help'])
        self.assertEqual(0, result.returncode)
        self.assertEqual('usage: uml [--runtime love] [--backend herdr] [--restart | ir | crap | mutate] [args]',
                         result.stdout.strip())
        result = self.run_entry(self.entry() + ['diagram with spaces.edn'])
        self.assertEqual(0, result.returncode, result.stderr)
        deadline = time.monotonic() + 10
        while not self.record.exists() and time.monotonic() < deadline:
            time.sleep(.05)
        self.assertTrue(self.record.exists(), result.stdout)
        received = json.loads(self.record.read_text(encoding='utf-8-sig'))
        self.assertEqual(['viewer', '--backend', 'herdr', 'diagram with spaces.edn'], received[-4:])
        self.assertFalse((self.project / '.uml-viewer/companion.edn').exists())

    def wait_for_record(self, result):
        deadline = time.monotonic() + 10
        while not self.record.exists() and time.monotonic() < deadline:
            time.sleep(.05)
        self.assertTrue(self.record.exists(), result.stdout + result.stderr)
        return json.loads(self.record.read_text(encoding='utf-8-sig'))

    def test_tool_root_override_selects_the_given_checkout(self):
        self.assertEqual(0, self.install().returncode)
        env = dict(self.env, UML_VIEWER_TOOL_ROOT=str(self.source))
        result = self.run_entry(self.entry() + ['diagram.edn'], env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        received = self.wait_for_record(result)
        self.assertEqual(os.path.normcase(os.path.normpath(str(self.source / 'love'))),
                         os.path.normcase(os.path.normpath(received[0])))
        self.assertFalse((self.project / '.uml-viewer/companion.edn').exists())

    def test_tool_root_override_must_contain_the_viewer(self):
        self.assertEqual(0, self.install().returncode)
        missing = self.root / 'not a checkout'
        missing.mkdir()
        result = self.run_entry(self.entry() + ['diagram.edn'],
                                env=dict(self.env, UML_VIEWER_TOOL_ROOT=str(missing)))
        self.assertNotEqual(0, result.returncode)
        self.assertIn('UML_VIEWER_TOOL_ROOT', result.stderr)
        self.assertIn('love/main.lua', result.stderr)
        self.assertFalse(self.record.exists())

    def test_unresolvable_love_executable_names_the_console_build(self):
        self.assertEqual(0, self.install().returncode)
        result = self.run_entry(self.entry() + ['diagram.edn'],
                                env=dict(self.env, UML_LOVE_EXECUTABLE=str(self.root / 'missing-love.exe')))
        self.assertNotEqual(0, result.returncode)
        self.assertIn('LOVE 11.5 is required', result.stderr)
        self.assertIn('lovec.exe', result.stderr)

    def test_silent_gui_love_build_reports_the_preflight_diagnostic(self):
        self.assertEqual(0, self.install().returncode)
        # Basename starts with `love` but not `lovec`, and prints nothing, like
        # the GUI-subsystem love.exe.
        silent = self.tools / 'love-gui.ps1'
        silent.write_text('exit 0\n')
        result = self.run_entry(self.entry() + ['diagram.edn'],
                                env=dict(self.env, UML_LOVE_EXECUTABLE=str(silent)))
        self.assertNotEqual(0, result.returncode)
        self.assertIn('Backend preflight returned invalid output; no viewer was launched.', result.stderr)
        self.assertIn('no stdout', result.stderr)
        self.assertIn('lovec.exe', result.stderr)
        self.assertFalse((self.project / '.uml-viewer/companion.edn').exists())

    def test_missing_tool_venv_reports_provisioning_steps(self):
        self.assertEqual(0, self.install().returncode)
        (self.project / '.uml-viewer/crapper/.venv/Scripts/python.exe').unlink()
        result = self.run_entry(self.entry() + ['crap'])
        self.assertNotEqual(0, result.returncode)
        self.assertIn('py -3 -m venv', result.stderr)
        self.assertIn('pip install -e', result.stderr)

    def test_background_launcher_invokes_powershell_cli_as_data(self):
        cli = self.root / 'love with spaces.ps1'
        cli.write_text('[IO.File]::WriteAllText((Join-Path (Get-Location) "script-argv.txt"), (ConvertTo-Json -InputObject @($args) -Compress))\nexit 17\n')
        manifest = self.root / 'launch.json'
        manifest.write_text(json.dumps({'executable': str(cli), 'cwd': str(self.project),
                                        'log': str(self.project / 'launch.log'),
                                        'argv': ['with spaces', 'literal;$(throw 1)', 'quoted "value"', '', 'first\nsecond'], 'powershell': True}))
        result = self.run_entry(['pwsh', '-NoProfile', '-File', str(REPO / 'scripts/uml-launch.ps1'), str(manifest)])
        self.assertEqual(17, result.returncode, result.stderr)
        self.assertEqual(['with spaces', 'literal;$(throw 1)', 'quoted "value"', '', 'first\nsecond'],
                         json.loads((self.project / 'script-argv.txt').read_text()))

    def test_windows_metric_process_boundary_uses_project_local_python_module(self):
        self.assertEqual(0, self.install().returncode)
        runner = self.root / 'windows boundary.ps1'
        runner.write_text('Set-Variable IsWindows -Value $true -Force\n& $env:TEST_ENTRY @args\nexit $LASTEXITCODE\n')
        env = dict(self.env, TEST_ENTRY=str(self.project / 'uml.ps1'))
        for command, tool in [('crap', 'crapper'), ('mutate', 'mutator')]:
            venv = self.project / '.uml-viewer' / tool / '.venv'
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
            result = self.run_entry(['pwsh', '-NoProfile', '-File', str(runner), command,
                                     'src/sample.lua'], cwd=self.root, env=env)
            self.assertEqual(19, result.returncode, result.stderr)
            forwarded = (['--full-tree'] if command == 'crap' else []) + ['src/sample.lua']
            self.assertEqual([str(self.project), '-m', tool, *forwarded],
                             (self.project / 'windows.txt').read_text().splitlines())
            self.assertIn('stdout', result.stdout)
            self.assertIn('stderr', result.stderr)


class InstallerBootstrap(unittest.TestCase):
    @unittest.skipUnless(shutil.which('sh'), 'POSIX shell required for standalone entry')
    def test_standalone_download_defaults_to_main_and_preserves_overrides(self):
        with tempfile.TemporaryDirectory(prefix='uml-bootstrap-') as directory:
            root = Path(directory)
            entry = root / 'get-uml-viewer'
            shutil.copyfile(REPO / 'scripts/get-uml-viewer', entry)
            tools = root / 'tools'
            tools.mkdir()
            record = root / 'download-url.txt'
            # Only the network and interpreter process boundaries are fixtures.
            (tools / 'curl').write_text(
                '#!/bin/sh\nprintf "%s" "$2" > "$BOOTSTRAP_RECORD"\n'
                'printf "fixture" > "$4"\n')
            (tools / 'pwsh').write_text('#!/bin/sh\nexit 0\n')
            shell = shutil.which('sh')
            subprocess.run([shell, '-c', 'chmod +x "$1" "$2"', 'fixture',
                            (tools / 'curl').as_posix(), (tools / 'pwsh').as_posix()], check=True)
            base_env = dict(os.environ, PATH=str(tools) + os.pathsep + os.environ['PATH'],
                            BOOTSTRAP_RECORD=record.as_posix())
            for name in ['UML_VIEWER_REF', 'UML_VIEWER_INSTALLER_URL']:
                base_env.pop(name, None)
            cases = [
                ({}, 'https://raw.githubusercontent.com/qinyuanj/uml-viewer/main/scripts/get-uml-viewer.ps1'),
                ({'UML_VIEWER_REF': 'release-test'},
                 'https://raw.githubusercontent.com/qinyuanj/uml-viewer/release-test/scripts/get-uml-viewer.ps1'),
                ({'UML_VIEWER_REF': 'release-test', 'UML_VIEWER_INSTALLER_URL': 'https://example.org/custom.ps1'},
                 'https://example.org/custom.ps1'),
            ]
            for overrides, expected in cases:
                with self.subTest(overrides=overrides):
                    result = subprocess.run([shell, entry.as_posix(), '--install-only'],
                                            cwd=root, env=dict(base_env, **overrides),
                                            capture_output=True, text=True)
                    self.assertEqual(0, result.returncode, result.stdout + result.stderr)
                    self.assertEqual(expected, record.read_text())


if __name__ == '__main__':
    unittest.main()
