"""Public LOVE restart identity on native Windows; the JVM runtime is removed."""
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
        self.temp = tempfile.TemporaryDirectory(prefix='uml-72-restart-')
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

    def write_launcher(self, runtime='love', pid=2147483647):
        record = {'pid': pid, 'started': '1', 'token': 'isolated-dead-launcher'}
        if runtime:
            record['runtime'] = runtime
        self.launcher.write_text(json.dumps(record), encoding='utf-8')

    def love_process(self, pings=8):
        """Owned native process whose image name is love, not the viewer itself."""
        image = self.root/'love.exe'
        system32 = Path(os.environ.get('SystemRoot', r'C:\Windows'))/'System32'
        shutil.copyfile(system32/'ping.exe', image)
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
        stub.write_text("if ($args -contains 'companion-preflight') { Write-Output 'herdr'; exit 0 }\n"
                        "$args | ConvertTo-Json -AsArray | Set-Content -LiteralPath $env:UML72_RECORD\n")
        return stub, record, {'UML_LOVE_EXECUTABLE': str(stub), 'UML72_RECORD': str(record)}

    def await_file(self, path, needle=None, timeout=25):
        deadline = time.monotonic()+timeout
        while time.monotonic() < deadline:
            if path.exists():
                text = path.read_text(encoding='utf-8-sig', errors='replace')
                if needle is None or needle in text:
                    return text
            time.sleep(.05)
        self.fail(f'timed out waiting for {path}')

    def test_default_restart_validates_identity_waits_and_relaunches_same_runtime(self):
        process, started = self.love_process(8)
        self.write_launcher('love')
        self.love_identity.write_text(json.dumps(
            {'pid': process.pid, 'started': started, 'runtime': 'love',
             'cwd': str(self.root).replace('\\', '/')}), encoding='utf-8')
        before = {'launcher': self.launcher.read_bytes(), 'identity': self.love_identity.read_bytes()}
        stub, record, env = self.love_stub()
        result = self.invoke('--restart', 'diagram with spaces.edn', env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertIn('Waiting for the old LOVE viewer', result.stdout)
        self.assertIsNotNone(process.poll(), 'restart returned before the old LOVE process exited')
        received = json.loads(self.await_file(record))
        self.assertEqual(['viewer', '--backend', 'herdr', '--restart', 'diagram with spaces.edn'], received[-5:])
        self.assertEqual(before['identity'], self.love_identity.read_bytes())

    def test_explicit_love_runtime_remains_accepted(self):
        stub, record, env = self.love_stub()
        result = self.invoke('--runtime', 'love', 'diagram.edn', env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        received = json.loads(self.await_file(record))
        self.assertEqual(['viewer', '--backend', 'herdr', 'diagram.edn'], received[-4:])

    def test_restart_refuses_missing_identity_without_replacement(self):
        self.write_launcher('love')
        before = self.launcher.read_bytes()
        result = self.invoke('--restart')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('LOVE viewer process identity', result.stderr)
        self.assertIn('cannot safely restart', result.stderr)
        self.assertEqual(before, self.launcher.read_bytes())
        self.assertFalse(list(self.directory.glob('launch-*.json')))
        self.assertFalse(self.love_identity.exists())

    def test_restart_refuses_bad_identity_before_waiting_for_a_live_launcher(self):
        process, started = self.love_process(10)
        launcher_started = filetime_ticks(os.getpid())
        self.launcher.write_text(json.dumps(
            {'pid': os.getpid(), 'started': launcher_started, 'runtime': 'love'}), encoding='utf-8')
        before = (self.launcher.read_bytes(), self.love_identity.read_bytes()
                  if self.love_identity.exists() else None)
        cases = [
            ('changed ticks', {'pid': process.pid, 'started': str(int(started)+1), 'runtime': 'love'}),
            ('missing identity', None),
        ]
        for name, record in cases:
            with self.subTest(name=name):
                if record is None:
                    self.love_identity.unlink(missing_ok=True)
                else:
                    self.love_identity.write_text(json.dumps(record), encoding='utf-8')
                began = time.monotonic()
                result = self.invoke('--restart')
                elapsed = time.monotonic()-began
                self.assertNotEqual(0, result.returncode)
                self.assertIn('cannot safely restart', result.stderr)
                self.assertNotIn('still running', result.stderr)
                self.assertLess(elapsed, 10, 'identity refusal must not wait for the live launcher')
                self.assertEqual(before[0], self.launcher.read_bytes())
                self.assertFalse(list(self.directory.glob('launch-*.json')))

    def test_restart_refuses_changed_pid_or_start_without_replacement(self):
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
                result = self.invoke('--restart')
                self.assertNotEqual(0, result.returncode)
                self.assertIn('cannot safely restart', result.stderr)
                self.assertNotIn('Waiting for the old LOVE viewer', result.stdout)
                self.assertEqual(original, self.launcher.read_bytes())
                self.assertEqual(before, self.love_identity.read_bytes())
                self.assertFalse(list(self.directory.glob('launch-*.json')))

    def test_legacy_jvm_launcher_runtime_is_refused(self):
        self.write_launcher('jvm')
        before = self.launcher.read_bytes()
        result = self.invoke('--restart')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('LOVE cutover', result.stderr)
        self.assertEqual(before, self.launcher.read_bytes())
        self.assertFalse(list(self.directory.glob('launch-*.json')))

    def test_legacy_viewer_jvm_record_is_not_recognized_as_identity(self):
        self.write_launcher('love')
        self.jvm_identity.write_text(json.dumps(
            {'pid': 2147483647, 'started': '2020-01-01T00:00:00.000Z'}), encoding='utf-8')
        before = {'launcher': self.launcher.read_bytes(), 'jvm': self.jvm_identity.read_bytes()}
        result = self.invoke('--restart')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('LOVE viewer process identity', result.stderr)
        self.assertNotIn('ambiguous', result.stderr.lower())
        self.assertEqual(before['launcher'], self.launcher.read_bytes())
        self.assertEqual(before['jvm'], self.jvm_identity.read_bytes())
        self.assertFalse(list(self.directory.glob('launch-*.json')))


if __name__ == '__main__':
    unittest.main()
