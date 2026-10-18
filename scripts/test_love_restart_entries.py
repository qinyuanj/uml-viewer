"""Public LOVE restart identity and cross-runtime refusal on native Windows."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time
import unittest

REPO = Path(__file__).resolve().parents[1]
EPOCH = 504911232000000000
NO_WINDOW = getattr(subprocess, 'CREATE_NO_WINDOW', 0)


def pwsh(command):
    return subprocess.run(['pwsh', '-NoProfile', '-Command', command], capture_output=True,
                          text=True, creationflags=NO_WINDOW)


def filetime_ticks(pid):
    result = pwsh(f'$p=Get-Process -Id {pid}; '
                  f'[string](([bigint]$p.StartTime.ToUniversalTime().Ticks)-[bigint]{EPOCH})')
    return result.stdout.strip()


def process_name(pid):
    result = pwsh(f'(Get-Process -Id {pid} -ErrorAction SilentlyContinue).ProcessName')
    return result.stdout.strip()


@unittest.skipUnless(os.name == 'nt', 'native Windows wrapper required')
class LoveRestartEntries(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='uml-69-restart-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.installed = self.root/'.uml-viewer/uml-viewer'
        (self.installed/'love').mkdir(parents=True)
        shutil.copytree(REPO/'scripts', self.installed/'scripts')
        self.directory = self.root/'.uml-viewer'
        self.launcher = self.directory/'viewer-process.json'
        self.love_identity = self.directory/'viewer-process-identity.json'
        self.jvm_identity = self.directory/'viewer-jvm.json'
        self.children = []

    def tearDown(self):
        for child in self.children:
            try:
                child.wait(timeout=20)
            except subprocess.TimeoutExpired:
                child.terminate()
                child.wait(timeout=10)

    def invoke(self, *args, env=None):
        environment = dict(os.environ)
        if env:
            environment.update(env)
        return subprocess.run(
            ['pwsh', '-NoProfile', '-File', str(self.installed/'scripts/uml-command.ps1'),
             str(self.root), *args],
            capture_output=True, text=True, env=environment, timeout=60, creationflags=NO_WINDOW)

    def write_launcher(self, runtime=None, pid=2147483647):
        record = {'pid': pid, 'started': '1', 'token': 'isolated-dead-launcher'}
        if runtime:
            record['runtime'] = runtime
        self.launcher.write_text(json.dumps(record), encoding='utf-8')

    def love_process(self, pings=8):
        """Owned native process whose image name is love, not the viewer itself."""
        image = self.root/'love.exe'
        shutil.copyfile(r'C:\Windows\System32\ping.exe', image)
        process = subprocess.Popen([str(image), '-n', str(pings), '127.0.0.1'],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   creationflags=NO_WINDOW)
        self.children.append(process)
        deadline = time.monotonic()+10
        while time.monotonic() < deadline:
            if process.poll() is not None:
                self.fail('fixture love process exited before identity was sampled')
            if process_name(process.pid) == 'love':
                return process, filetime_ticks(process.pid)
            time.sleep(.05)
        self.fail('fixture process did not acquire the love image name')

    def love_stub(self):
        stub = self.root/'love stub.ps1'
        record = self.root/'love-received.json'
        stub.write_text('$args | ConvertTo-Json -AsArray | Set-Content -LiteralPath $env:UML69_RECORD\n')
        return stub, record, {'UML_LOVE_EXECUTABLE': str(stub), 'UML69_RECORD': str(record)}

    def clojure_stub(self):
        stub = self.root/'clojure.ps1'
        record = self.root/'clojure-received.jsonl'
        stub.write_text(
            '$args | ConvertTo-Json -AsArray -Compress | Add-Content -LiteralPath $env:UML69_RECORD\n'
            'if ($args -contains "uml-viewer.adapters.companion") { Write-Output "herdr" }\n'
            '$global:LASTEXITCODE = 0\n')
        bin_dir = self.root/'bin'
        bin_dir.mkdir()
        modules = self.root/'no-modules'
        modules.mkdir()
        shutil.copyfile(stub, bin_dir/'clojure.ps1')
        # Importing ClojureTools would shadow the PATH fixture with the real
        # module, so make that import fail in this isolated project.
        return record, {'PATH': str(bin_dir)+os.pathsep+os.environ.get('PATH', ''),
                        'PSModulePath': str(modules), 'UML69_RECORD': str(record)}

    def await_file(self, path, needle=None, timeout=25):
        deadline = time.monotonic()+timeout
        while time.monotonic() < deadline:
            if path.exists():
                text = path.read_text(encoding='utf-8-sig', errors='replace')
                if needle is None or needle in text:
                    return text
            time.sleep(.05)
        self.fail(f'timed out waiting for {path}')

    def test_love_restart_validates_identity_waits_and_relaunches_same_runtime(self):
        process, started = self.love_process(8)
        self.write_launcher('love')
        self.love_identity.write_text(json.dumps(
            {'pid': process.pid, 'started': started, 'runtime': 'love',
             'cwd': str(self.root).replace('\\', '/')}), encoding='utf-8')
        before = {'launcher': self.launcher.read_bytes(), 'identity': self.love_identity.read_bytes()}
        stub, record, env = self.love_stub()
        result = self.invoke('--runtime', 'love', '--restart', 'diagram with spaces.edn', env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn('Waiting for the old LOVE viewer', result.stdout)
        self.assertIsNotNone(process.poll(), 'restart returned before the old LOVE process exited')
        received = json.loads(self.await_file(record))
        self.assertEqual(['viewer', '--restart', 'diagram with spaces.edn'], received[-3:])
        # A successful relaunch writes a new launcher record, but validation and
        # waiting never rewrite the old runtime identity.
        self.assertEqual(before['identity'], self.love_identity.read_bytes())

    def test_love_restart_refuses_missing_identity_without_replacement(self):
        self.write_launcher('love')
        before = self.launcher.read_bytes()
        result = self.invoke('--runtime', 'love', '--restart')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('LOVE viewer process identity', result.stderr)
        self.assertIn('cannot safely restart', result.stderr)
        self.assertEqual(before, self.launcher.read_bytes())
        self.assertFalse(list(self.directory.glob('launch-*.json')))
        self.assertFalse(self.love_identity.exists())

    def test_love_restart_refuses_changed_pid_or_start_without_replacement(self):
        process, started = self.love_process(10)
        self.write_launcher('love')
        original = self.launcher.read_bytes()
        python_started = filetime_ticks(os.getpid())
        cases = [
            ('different creation ticks', {'pid': process.pid, 'started': str(int(started)+1),
                                          'runtime': 'love', 'cwd': str(self.root).replace('\\', '/')}),
            ('non-love process name', {'pid': os.getpid(), 'started': python_started,
                                       'runtime': 'love', 'cwd': str(self.root).replace('\\', '/')}),
            ('missing runtime', {'pid': process.pid, 'started': started}),
            ('malformed start', {'pid': process.pid, 'started': 'garbage', 'runtime': 'love'}),
        ]
        for name, record in cases:
            with self.subTest(name=name):
                self.love_identity.write_text(json.dumps(record), encoding='utf-8')
                before = self.love_identity.read_bytes()
                result = self.invoke('--runtime', 'love', '--restart')
                self.assertNotEqual(0, result.returncode)
                self.assertIn('cannot safely restart', result.stderr)
                self.assertNotIn('Waiting for the old LOVE viewer', result.stdout)
                self.assertEqual(original, self.launcher.read_bytes())
                self.assertEqual(before, self.love_identity.read_bytes())
                self.assertFalse(list(self.directory.glob('launch-*.json')))

    def test_cross_runtime_jvm_session_refuses_love_restart_without_ownership_changes(self):
        self.write_launcher('jvm')
        self.jvm_identity.write_text(json.dumps(
            {'pid': 2147483647, 'started': '2020-01-01T00:00:00.000Z'}), encoding='utf-8')
        before = {'launcher': self.launcher.read_bytes(), 'jvm': self.jvm_identity.read_bytes()}
        result = self.invoke('--runtime', 'love', '--restart')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('Cannot restart a JVM viewer as love', result.stderr)
        self.assertIn('--runtime jvm', result.stderr)
        self.assertEqual(before['launcher'], self.launcher.read_bytes())
        self.assertEqual(before['jvm'], self.jvm_identity.read_bytes())
        self.assertFalse(list(self.directory.glob('launch-*.json')))

    def test_cross_runtime_love_session_refuses_jvm_restart_without_ownership_changes(self):
        self.write_launcher('love')
        self.love_identity.write_text(json.dumps(
            {'pid': 2147483647, 'started': '4294967296', 'runtime': 'love',
             'cwd': str(self.root).replace('\\', '/')}), encoding='utf-8')
        before = {'launcher': self.launcher.read_bytes(), 'love': self.love_identity.read_bytes()}
        record, env = self.clojure_stub()
        result = self.invoke('--restart', env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('Cannot restart a LOVE viewer as jvm', result.stderr)
        self.assertIn('--runtime love', result.stderr)
        self.assertEqual(before['launcher'], self.launcher.read_bytes())
        self.assertEqual(before['love'], self.love_identity.read_bytes())
        self.assertFalse(list(self.directory.glob('launch-*.json')))
        lines = record.read_text(encoding='utf-8').splitlines() if record.exists() else []
        self.assertFalse([line for line in lines if 'uml-viewer.main.uml-viewer' in line],
                         'refused cross-runtime restart must not launch a viewer')

    def test_legacy_dual_identity_is_ambiguous_and_refused(self):
        self.write_launcher()
        self.jvm_identity.write_text(json.dumps({'pid': 2147483647, 'started': '2020-01-01T00:00:00.000Z'}))
        self.love_identity.write_text(json.dumps(
            {'pid': 2147483647, 'started': '4294967296', 'runtime': 'love'}))
        before = (self.launcher.read_bytes(), self.jvm_identity.read_bytes(),
                  self.love_identity.read_bytes())
        result = self.invoke('--runtime', 'love', '--restart')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('ambiguous', result.stderr.lower())
        self.assertFalse(list(self.directory.glob('launch-*.json')))
        self.assertEqual(before, (self.launcher.read_bytes(), self.jvm_identity.read_bytes(),
                                  self.love_identity.read_bytes()))

    def test_jvm_restart_uses_recorded_runtime_despite_a_stale_love_identity(self):
        self.write_launcher('jvm')
        self.jvm_identity.write_text(json.dumps(
            {'pid': 2147483647, 'started': '2020-01-01T00:00:00.000Z'}), encoding='utf-8')
        stale = json.dumps({'pid': 2147483647, 'started': '4294967296', 'runtime': 'love'}).encode()
        self.love_identity.write_bytes(stale)
        record, env = self.clojure_stub()
        result = self.invoke('--restart', env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        text = self.await_file(record, 'uml-viewer.main.uml-viewer')
        viewer = [json.loads(line) for line in text.splitlines()
                  if 'uml-viewer.main.uml-viewer' in line][-1]
        self.assertEqual(['--backend', 'herdr', '--restart'], viewer[-3:])
        self.assertEqual(stale, self.love_identity.read_bytes())


if __name__ == '__main__':
    unittest.main()
