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

    def test_ir_output_is_byte_identical_across_runs(self):
        outputs = set()
        for _ in range(8):
            result = self.entry('ir', 'policy.edn', 'stable.edn')
            self.assertEqual(0, result.returncode, result.stderr)
            outputs.add((self.root / 'stable.edn').read_bytes())
        self.assertEqual(1, len(outputs), 'IR bytes changed between identical runs')

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


    def test_language_detection_from_markers(self):
        # No :lang and no :sources in the policy: the marker file in the tree
        # decides the scanner, and the result must carry the detected language.
        markers = {
            'lua': ('.', 'src/a.lua', 'local M={}\nfunction M.hello() return true end\nreturn M\n'),
            'python': ('pyproject.toml', 'src/a.py', 'def hello():\n    return True\n'),
            'typescript': ('package.json', 'src/a.ts', 'export function hello(): boolean { return true; }\n'),
            'rust': ('Cargo.toml', 'src/lib.rs', 'pub fn hello() -> bool { true }\n'),
        }
        for language, (marker, relative, source) in markers.items():
            with self.subTest(language=language):
                for stale in list(self.root.glob('src/*')):
                    stale.unlink()
                for other in ('pyproject.toml', 'package.json', 'Cargo.toml'):
                    (self.root / other).unlink(missing_ok=True)
                if marker != '.':
                    (self.root / marker).write_text('')
                path = self.root / relative
                path.parent.mkdir(exist_ok=True)
                path.write_text(source)
                (self.root / 'detect.edn').write_text('{:src "src" :prefix "app"}')
                name = language + '-detected.edn'
                result = self.entry('ir', 'detect.edn', name)
                self.assertEqual(0, result.returncode, result.stderr)
                document = (self.root / name).read_text()
                self.assertRegex(document, r':lang\s+:' + language + r'\b')
                self.assertRegex(document, r':classes\s+\[\s*\{')
                self.assertIn('hello', document)

    def test_ambiguous_markers_require_an_explicit_language(self):
        (self.root / 'src').mkdir(exist_ok=True)
        (self.root / 'src/a.lua').write_text('return {}\n')
        (self.root / 'Cargo.toml').write_text('')
        (self.root / 'ambiguous.edn').write_text('{:src "src" :prefix "app"}')
        result = self.entry('ir', 'ambiguous.edn', 'ambiguous-result.edn')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('declare explicit :lang or :sources', result.stderr)
        self.assertFalse((self.root / 'ambiguous-result.edn').exists())

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


    def test_reinstall_is_idempotent_and_provisions_all_three_tools(self):
        # A repo fixture the installer clones: the real installer path, not a
        # stub entry. Only the remote source is redirected to the checkout.
        source = self.root / 'viewer source'
        shutil.copytree(REPO, source, ignore=shutil.ignore_patterns(
            '.git', '.uml-viewer', 'target', 'coverage', '__pycache__'))
        subprocess.run(['git', 'init', '-q', '-b', 'main', str(source)], check=True)
        subprocess.run(['git', '-C', str(source), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(source), '-c', 'user.name=Test',
                        '-c', 'user.email=test@example.org', 'commit', '-qm', 'fixture'],
                       check=True, capture_output=True)
        self.write_project_policy()
        tools = {}
        for tool in ('crapper', 'mutator', 'dryer'):
            tools[tool] = self.tool_fixture(self.root / f'{tool} source', tool)
        env = dict(self.env, UML_VIEWER_TOOL_ROOT='', UML_VIEWER_REPO_URL=str(source),
                   UML_VIEWER_REF='main',
                   **{f'{tool.upper()}_REPO_URL': str(path) for tool, path in tools.items()},
                   **{f'{tool.upper()}_REF': 'main' for tool in tools})
        (self.root / '.gitignore').write_text('')
        policy = (self.root / 'policy.edn').read_bytes()
        for _ in range(2):
            result = subprocess.run([str(REPO / 'scripts/get-uml-viewer'), '--install-only'],
                                    cwd=self.root, env=env, capture_output=True, text=True, timeout=900)
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            self.assertTrue((self.root / 'uml').is_file())
            self.assertEqual(policy, (self.root / 'policy.edn').read_bytes())
        self.assertEqual(1, (self.root / '.gitignore').read_text().count('# BEGIN UML-VIEWER'))
        for tool in ('crapper', 'mutator', 'dryer'):
            directory = self.root / '.uml-viewer' / tool
            self.assertTrue((directory / '.git').exists(), tool)
            self.assertTrue((directory / '.venv/bin/python').exists(), tool)
        # Mutator needs a compatible crapper beside it.
        check = subprocess.run([str(self.root / '.uml-viewer/mutator/.venv/bin/python'),
                                '-I', '-m', 'pip', 'check'], capture_output=True, text=True)
        self.assertEqual(0, check.returncode, check.stdout + check.stderr)
        installed = json.loads((self.root / '.uml-viewer/install.json').read_text())
        self.assertEqual('love', 'love' if installed['love'] else 'missing')
        self.assertTrue(all(installed['repositories'][name]['commit'] for name in
                            ('uml-viewer', 'crapper', 'mutator', 'dryer')))

    def tool_fixture(self, directory, name):
        """A minimal installable Python package, enough for a real venv."""
        directory.mkdir(parents=True)
        (directory / 'pyproject.toml').write_text(
            f'[project]\nname = "{name}"\nversion = "0.1.0"\nrequires-python = ">=3.11"\n'
            '[build-system]\nrequires = ["setuptools>=68"]\nbuild-backend = "setuptools.build_meta"\n')
        package = directory / name
        package.mkdir()
        (package / '__init__.py').write_text('')
        (package / '__main__.py').write_text('raise SystemExit(0)\n')
        subprocess.run(['git', 'init', '-q', '-b', 'main', str(directory)], check=True)
        subprocess.run(['git', '-C', str(directory), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(directory), '-c', 'user.name=Test',
                        '-c', 'user.email=test@example.org', 'commit', '-qm', 'fixture'],
                       check=True, capture_output=True)
        return directory

    def write_project_policy(self):
        (self.root / 'src').mkdir(exist_ok=True)
        (self.root / 'src/a.lua').write_text('local M={}\nfunction M.hello() return "hello" end\nreturn M\n')
        (self.root / 'policy.edn').write_text(self.policy)

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


    def test_install_defaults_to_main_and_accepts_a_fixed_commit(self):
        source = self.root / 'viewer source'
        shutil.copytree(REPO, source, ignore=shutil.ignore_patterns(
            '.git', '.uml-viewer', 'target', 'coverage', '__pycache__'))
        subprocess.run(['git', 'init', '-q', '-b', 'main', str(source)], check=True)
        subprocess.run(['git', '-C', str(source), 'add', '.'], check=True)
        subprocess.run(['git', '-C', str(source), '-c', 'user.name=Test',
                        '-c', 'user.email=test@example.org', 'commit', '-qm', 'fixture'],
                       check=True, capture_output=True)
        first = subprocess.run(['git', '-C', str(source), 'rev-parse', 'HEAD'],
                               check=True, capture_output=True, text=True).stdout.strip()
        marker = source / 'scripts/uml_posix.py'
        marker.write_text(marker.read_text() + '\n# later revision\n')
        subprocess.run(['git', '-C', str(source), 'add', 'scripts/uml_posix.py'], check=True)
        subprocess.run(['git', '-C', str(source), '-c', 'user.name=Test',
                        '-c', 'user.email=test@example.org', 'commit', '-qm', 'later'],
                       check=True, capture_output=True)
        tools = {tool: self.tool_fixture(self.root / f'{tool} source', tool)
                 for tool in ('crapper', 'mutator', 'dryer')}
        base = dict(self.env, UML_VIEWER_TOOL_ROOT='', UML_VIEWER_REPO_URL=str(source),
                    **{f'{tool.upper()}_REPO_URL': str(path) for tool, path in tools.items()})
        for name in ('UML_VIEWER_REF', 'CRAPPER_REF', 'MUTATOR_REF', 'DRYER_REF'):
            base.pop(name, None)
        # Default ref is main for every repository, including the tools.
        result = subprocess.run([str(REPO / 'scripts/get-uml-viewer'), '--install-only'],
                                cwd=self.root, env=base, capture_output=True, text=True, timeout=900)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        installed = json.loads((self.root / '.uml-viewer/install.json').read_text())
        self.assertEqual('main', installed['repositories']['crapper']['ref'])
        # An explicit commit pins the viewer even when main has moved on.
        pinned = dict(base, UML_VIEWER_REF=first)
        result = subprocess.run([str(REPO / 'scripts/get-uml-viewer'), '--install-only'],
                                cwd=self.root, env=pinned, capture_output=True, text=True, timeout=900)
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        installed = json.loads((self.root / '.uml-viewer/install.json').read_text())
        self.assertEqual(first, installed['repositories']['uml-viewer']['commit'])
        frozen = self.root / '.uml-viewer/uml-viewer/scripts/uml_posix.py'
        self.assertNotIn('later revision', frozen.read_text())
        commits = subprocess.run(['git', '-C', str(self.root / '.uml-viewer/uml-viewer'),
                                  'rev-list', '--all', '--count'],
                                 check=True, capture_output=True, text=True).stdout.strip()
        self.assertEqual('1', commits)

    def test_missing_tool_venv_reports_the_provisioning_step(self):
        state = self.root / '.uml-viewer/mutator'
        state.mkdir(parents=True)
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {'sourceRoots': ['src']}}))
        result = self.entry('mutate', 'src/a.lua')
        self.assertNotEqual(0, result.returncode)
        self.assertIn('get-uml-viewer', result.stderr)

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
