"""POSIX 公共入口的安全 restart：身份核对、真实退出等待与拒绝规则。

只通过公共 ./uml --restart 观察行为；进程用真实 /proc 身份。
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


def start_identity(pid):
    try:
        text = Path(f'/proc/{pid}/stat').read_text()
    except OSError:
        return None
    return text.rsplit(')', 1)[-1].split()[19]


class RestartEntries(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='uml restart ')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.state = self.root / '.uml-viewer'
        self.state.mkdir()
        shutil.copy2(REPO / 'scripts/uml', self.root / 'uml')
        (self.root / 'uml').chmod(0o755)
        self.env = dict(os.environ, UML_VIEWER_TOOL_ROOT=str(REPO))
        self.env.pop('UML_VIEWER_PROJECT_ROOT', None)

    def invoke(self, *args):
        return subprocess.run([str(self.root / 'uml'), *args], cwd=self.root, env=self.env,
                              capture_output=True, text=True, timeout=60)

    def write(self, name, record):
        (self.state / name).write_text(json.dumps(record) + '\n')

    def running_sleeper(self):
        # A live process whose recorded identity can never match: the wrapper
        # must refuse it rather than take over an unrelated process.
        sleeper = subprocess.Popen(['sleep', '60'])
        self.addCleanup(sleeper.wait, 10)
        self.addCleanup(sleeper.terminate)
        return sleeper

    def test_restart_without_a_launcher_record_is_refused(self):
        result = self.invoke('--restart', 'diagram.edn')
        self.assertEqual(1, result.returncode)
        self.assertIn('launcher 进程记录', result.stderr)

    def test_restart_refuses_a_reused_pid(self):
        sleeper = self.running_sleeper()
        self.write('viewer-process.json',
                   {'pid': sleeper.pid, 'started': '1', 'runtime': 'rust'})
        result = self.invoke('--restart', 'diagram.edn')
        self.assertEqual(1, result.returncode)
        self.assertIn('身份已变化', result.stderr)
        self.assertIsNone(sleeper.poll(), 'a refused restart must not touch the live process')

    def test_restart_refuses_an_unknown_runtime(self):
        sleeper = self.running_sleeper()
        self.write('viewer-process.json',
                   {'pid': sleeper.pid, 'started': start_identity(sleeper.pid), 'runtime': 'jvm'})
        result = self.invoke('--restart', 'diagram.edn')
        self.assertEqual(1, result.returncode)
        self.assertIn('运行时', result.stderr)

    def test_restart_refuses_a_removed_love_record(self):
        # #144: a session recorded by the retired LOVE viewer must be refused
        # instead of being taken over by the Rust viewer.
        sleeper = self.running_sleeper()
        self.write('viewer-process.json',
                   {'pid': sleeper.pid, 'started': start_identity(sleeper.pid), 'runtime': 'love'})
        result = self.invoke('--restart', 'diagram.edn')
        self.assertEqual(1, result.returncode)
        self.assertIn('运行时', result.stderr)
        self.assertIn('love', result.stderr)
        self.assertIsNone(sleeper.poll(), 'a refused restart must not touch the live process')

    def test_restart_refuses_a_malformed_record(self):
        self.write('viewer-process.json', {'pid': 'not-a-pid', 'started': 'abc'})
        result = self.invoke('--restart', 'diagram.edn')
        self.assertEqual(1, result.returncode)
        self.assertIn('身份记录无效', result.stderr)


    def test_restart_refuses_unverifiable_viewer_identity_without_waiting(self):
        # Ported from the retired Windows suite: a changed viewer identity and
        # a missing identity both refuse before any wait, without touching the
        # live launcher or writing a launch record.
        sleeper = self.running_sleeper()
        # A launcher that has already exited: the wait for it returns at once,
        # so only the viewer identity can refuse the restart.
        exited = subprocess.Popen(['true'])
        exited.wait()
        launcher = {'pid': exited.pid, 'started': '1', 'runtime': 'rust'}
        self.write('viewer-process.json', launcher)
        before = (self.state / 'viewer-process.json').read_bytes()
        identity = self.state / 'viewer-process-identity.json'
        for name, record in (
            ('changed identity', {'pid': sleeper.pid, 'started': '1', 'runtime': 'rust'}),
            ('invalid identity', None),
        ):
            with self.subTest(name=name):
                if record is None:
                    identity.write_text('{"pid": 1}\n')
                else:
                    self.write('viewer-process-identity.json', record)
                began = time.monotonic()
                result = self.invoke('--restart', 'diagram.edn')
                self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                self.assertIn('身份', result.stderr)
                self.assertNotIn('仍在运行', result.stderr)
                self.assertLess(time.monotonic() - began, 10,
                                'an identity refusal must not wait on the live launcher')
                self.assertEqual(before, (self.state / 'viewer-process.json').read_bytes())
                self.assertEqual([], list(self.state.glob('launch-*.json')))


if __name__ == '__main__':
    unittest.main()
