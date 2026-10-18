import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]

class LoveRuntimeEntries(unittest.TestCase):
    def invoke(self, *args, env=None):
        return subprocess.run(['pwsh', '-NoProfile', '-File', str(REPO/'scripts/uml-command.ps1'), str(self.root), *args], capture_output=True, text=True, env=env, timeout=15)

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='uml-62-wrapper-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        tool = self.root/'.uml-viewer/uml-viewer'
        (tool/'love').mkdir(parents=True)
        shutil.copytree(REPO/'scripts', tool/'scripts')

    def test_runtime_rejects_unknown_or_missing_values(self):
        for args in [('--runtime',), ('--runtime', 'invalid')]:
            result = self.invoke(*args)
            self.assertNotEqual(0, result.returncode)
            self.assertIn('runtime', result.stderr.lower())

    def test_love_missing_has_specific_diagnostic_before_clojure(self):
        env = dict(os.environ, UML_LOVE_EXECUTABLE=str(self.root/'missing-love.exe'))
        result = self.invoke('--runtime', 'love', 'diagram.edn', env=env)
        self.assertNotEqual(0, result.returncode)
        self.assertIn('LOVE', result.stderr)
        self.assertNotIn('Clojure', result.stderr)
        self.assertFalse((self.root/'.uml-viewer/viewer-process.json').exists())

    def test_love_dispatch_bypasses_clojure_and_companion(self):
        stub = self.root/'love stub.ps1'
        record = self.root/'received.json'
        stub.write_text("$args | ConvertTo-Json -AsArray | Set-Content -LiteralPath $env:UML62_RECORD\n")
        env = dict(os.environ, UML_LOVE_EXECUTABLE=str(stub), UML62_RECORD=str(record))
        result = self.invoke('--runtime', 'love', 'diagram with spaces.edn', env=env)
        self.assertEqual(0, result.returncode, result.stderr)
        # Detached launcher is an owned fixture process and should finish promptly.
        import time
        deadline = time.monotonic()+10
        while not record.exists() and time.monotonic()<deadline:
            time.sleep(.05)
        self.assertTrue(record.exists(), result.stdout)
        received = json.loads(record.read_text(encoding='utf-8-sig'))
        self.assertEqual(['viewer', 'diagram with spaces.edn'], received[-2:])
        self.assertFalse((self.root/'.uml-viewer/companion.edn').exists())

if __name__ == '__main__':
    unittest.main()
