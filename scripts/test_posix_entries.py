"""公共 POSIX 入口验收；使用真实 LOVE 和隔离源码，不启动 companion。"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class PosixEntries(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='uml 空格项目 ')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        shutil.copy2(REPO / 'scripts/uml', self.root / 'uml')
        (self.root / 'src').mkdir()
        (self.root / 'src/a.lua').write_text('local M={}\nfunction M.hello() return "hello" end\nreturn M\n')
        self.policy = '{:lang :lua :src "src" :prefix "src" :proposals [{:id :saved :name "保留" :layers []}]}'
        (self.root / 'policy.edn').write_text(self.policy)
        self.env = dict(os.environ, UML_VIEWER_TOOL_ROOT=str(REPO))

    def entry(self, *args):
        return subprocess.run([str(self.root / 'uml'), *args], cwd=self.root, env=self.env,
                              text=True, capture_output=True, timeout=30)

    def test_real_headless_ir_with_unicode_spaces_and_repeat(self):
        for _ in range(2):
            result = self.entry('ir', 'policy.edn', '结果 图.edn')
            self.assertEqual(0, result.returncode, result.stderr)
            text = (self.root / '结果 图.edn').read_text()
            self.assertIn('hello', text)
            self.assertIn('src.a', text)
            self.assertIn('保留', text)
            self.assertEqual(self.policy, (self.root / 'policy.edn').read_text())

    def test_four_real_scanners(self):
        samples = {
            'lua': ('sample.lua', 'local M={}\nfunction M.hello() return true end\nreturn M\n'),
            'python': ('sample.py', 'def hello():\n    return True\n'),
            'typescript': ('sample.ts', 'export function hello(): boolean { return true; }\n'),
            'rust': ('lib.rs', 'pub fn hello() -> bool { true }\n'),
        }
        for language, (filename, source) in samples.items():
            with self.subTest(language=language):
                folder = self.root / language
                folder.mkdir()
                (folder / filename).write_text(source)
                (self.root / 'scanner.edn').write_text('{:lang :' + language + ' :src "' + language + '" :prefix "demo"}')
                result = self.entry('ir', 'scanner.edn', language + '.edn')
                self.assertEqual(0, result.returncode, result.stderr)
                self.assertIn('hello', (self.root / (language + '.edn')).read_text())

    def test_ir_matches_existing_language_goldens(self):
        fixtures = [('rust-fixture', 'fixtures/jvm-goldens/rust.ir.edn')]
        for fixture, golden in fixtures:
            with self.subTest(fixture=fixture):
                output = self.root / 'golden-result.edn'
                result = self.entry('ir', '--root', str(REPO / fixture), '--policy', 'policy.edn', '--output', str(output))
                self.assertEqual(0, result.returncode, result.stderr)
                comparison = subprocess.run(['lua', str(REPO / 'scripts/compare_ir.lua'), str(output), str(REPO / golden)],
                                            cwd=REPO, capture_output=True, text=True)
                self.assertEqual(0, comparison.returncode, comparison.stdout + comparison.stderr)

    def test_default_policy_selection(self):
        result = self.entry('ir')
        self.assertEqual(0, result.returncode, result.stderr)
        self.assertTrue((self.root / 'examples/uml-viewer.edn').exists())

    def test_ir_failure_preserves_previous_output(self):
        output = self.root / 'result.edn'
        output.write_text('previous')
        result = self.entry('ir', 'missing.edn', 'result.edn')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('IR generation failed', result.stderr)
        self.assertEqual('previous', output.read_text())

    def test_symlink_sources_fail_without_following_cycle(self):
        (self.root / 'src/cycle').symlink_to(self.root / 'src', target_is_directory=True)
        result = self.entry('ir', 'policy.edn', 'result.edn')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('symbolic link', result.stderr)
        self.assertFalse((self.root / 'result.edn').exists())

    def configured(self, codes=(0, 3)):
        worker = self.root / '质量 worker.py'
        worker.write_text('import json,sys\nfrom pathlib import Path\nPath("argv.json").write_text(json.dumps(sys.argv[1:]))\nraise SystemExit(3)\n')
        operation = {'argv': [sys.executable, str(worker)], 'forwardArgs': 'append',
                     'validResultExitCodes': list(codes)}
        crap = dict(operation, scope='targets')
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1, 'operations': {'crap': crap, 'mutate': operation}}))

    def test_configured_quality_real_exit_and_literal_argv(self):
        self.configured()
        result = self.entry('mutate', '带 空格.lua', '$(touch injected)')
        self.assertEqual(3, result.returncode, result.stderr)
        self.assertEqual(['带 空格.lua', '$(touch injected)'], json.loads((self.root / 'argv.json').read_text()))
        record = json.loads((self.root / '.uml-viewer/quality-result.json').read_text())
        self.assertTrue(record['validResult'])
        self.assertEqual(3, record['exitCode'])
        self.assertEqual(str(self.root.resolve()), record['projectRoot'])
        self.assertFalse((self.root / 'injected').exists())

    def test_invalid_config_removes_stale_result(self):
        state = self.root / '.uml-viewer'
        state.mkdir()
        (state / 'quality-result.json').write_text('{"validResult":true}')
        (self.root / 'uml-quality.json').write_text('{"version":false}')
        result = self.entry('crap')
        self.assertEqual(1, result.returncode)
        self.assertFalse((state / 'quality-result.json').exists())

    def test_undeclared_exit_three_is_invalid(self):
        self.configured((0,))
        result = self.entry('mutate', 'src/a.lua')
        self.assertEqual(3, result.returncode)
        self.assertFalse(json.loads((self.root / '.uml-viewer/quality-result.json').read_text())['validResult'])

    def test_install_refuses_existing_ownership(self):
        state = self.root / '.uml-viewer'
        state.mkdir()
        ownership = state / 'companion.edn'
        ownership.write_text('{:backend :unknown :pid 123}')
        result = subprocess.run([str(REPO / 'scripts/get-uml-viewer'), '--install-only'],
                                cwd=self.root, env=self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(1, result.returncode)
        self.assertIn('保留已有运行态', result.stderr)
        self.assertEqual('{:backend :unknown :pid 123}', ownership.read_text())

    def test_installer_preserves_dirty_tool_checkout(self):
        directory = self.root / '.uml-viewer/crapper'
        directory.mkdir(parents=True)
        subprocess.run(['git', 'init', '-q', str(directory)], check=True)
        user_file = directory / 'unique-work.txt'
        user_file.write_text('用户未提交内容')
        (self.root / '.uml-viewer/uml-viewer').symlink_to(REPO, target_is_directory=True)
        result = subprocess.run([str(REPO / 'scripts/get-uml-viewer'), '--install-only'],
                                cwd=self.root, env=self.env, capture_output=True, text=True, timeout=30)
        self.assertEqual(1, result.returncode)
        self.assertIn('本地修改', result.stderr)
        self.assertEqual('用户未提交内容', user_file.read_text())

    def test_standard_mutation_is_serial_per_file(self):
        directory = self.root / '.uml-viewer/mutator/.venv/bin'
        directory.mkdir(parents=True)
        python = directory / 'python'
        python.write_text('#!/usr/bin/python3\nimport json,sys\nfrom pathlib import Path\np=Path("calls.jsonl")\nwith p.open("a") as f: f.write(json.dumps(sys.argv[1:])+"\\n")\nraise SystemExit(3)\n')
        python.chmod(0o755)
        (self.root / 'src/b.lua').write_text('return {}')
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {'sourceRoots': ['src']}}))
        result = self.entry('mutate', 'src/a.lua', 'src/b.lua')
        self.assertEqual(3, result.returncode, result.stderr)
        calls = [json.loads(line) for line in (self.root / 'calls.jsonl').read_text().splitlines()]
        self.assertEqual([['-m', 'mutator', 'src/a.lua'], ['-m', 'mutator', 'src/b.lua']], calls)
        self.assertTrue(json.loads((self.root / '.uml-viewer/quality-result.json').read_text())['validResult'])

    def test_installer_help_and_missing_dependency(self):
        entry = str(REPO / 'scripts/get-uml-viewer')
        result = subprocess.run([entry, '--help'], capture_output=True, text=True)
        self.assertEqual(0, result.returncode)
        env = dict(self.env, UML_LOVE_EXECUTABLE='/missing/love')
        result = subprocess.run([entry, '--install-only'], cwd=self.root, env=env, capture_output=True, text=True)
        self.assertEqual(1, result.returncode)
        self.assertIn('LOVE 11.5', result.stderr)
        self.assertFalse((self.root / '.uml-viewer').exists())


if __name__ == '__main__':
    unittest.main()
