"""公共 POSIX 入口验收；使用真实 Rust 二进制和隔离源码，不启动 companion。"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
import unittest.mock

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
                comparison = subprocess.run(
                    [sys.executable, '-I', str(REPO / 'scripts/compare_ir.py'), str(output),
                     str(REPO / golden)],
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
        cargo, cargo_calls = self.cargo_stub()
        env = dict(self.env, UML_VIEWER_TOOL_ROOT='', UML_VIEWER_REPO_URL=str(source),
                   UML_VIEWER_REF='main', UML_CARGO_EXECUTABLE=str(cargo),
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
        # #143: each install ran the pinned release build in the checkout, and
        # the binary the rust runtime dispatch expects exists afterwards.
        builds = [line.rsplit('|', 1)[-1] for line in cargo_calls.read_text().splitlines()]
        self.assertEqual(['build --release --locked'] * 2, builds)
        self.assertTrue(all('.uml-viewer/uml-viewer' in line
                            for line in cargo_calls.read_text().splitlines()))
        binary = self.root / '.uml-viewer/uml-viewer/target/release/uml-viewer'
        self.assertTrue(binary.is_file() and os.access(binary, os.X_OK))
        for tool in ('crapper', 'mutator', 'dryer'):
            directory = self.root / '.uml-viewer' / tool
            self.assertTrue((directory / '.git').exists(), tool)
            self.assertTrue((directory / '.venv/bin/python').exists(), tool)
        # Mutator needs a compatible crapper beside it.
        check = subprocess.run([str(self.root / '.uml-viewer/mutator/.venv/bin/python'),
                                '-I', '-m', 'pip', 'check'], capture_output=True, text=True)
        self.assertEqual(0, check.returncode, check.stdout + check.stderr)
        installed = json.loads((self.root / '.uml-viewer/install.json').read_text())
        # #144: the installer no longer provisions or records LOVE/cimgui.
        self.assertNotIn('love', installed)
        self.assertNotIn('cimgui', str(installed))
        self.assertEqual(str(binary), installed['rust'])
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

    def test_installer_refuses_hosts_other_than_wsl_x86_64(self):
        """Only WSL2 x86_64 is delivered; macOS is not committed (ADR 0007)."""
        import importlib.util
        from unittest import mock
        specification = importlib.util.spec_from_file_location(
            'uml_posix', REPO / 'scripts/uml_posix.py')
        installer = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(installer)
        hosts = {('Darwin', 'arm64', '25.0.0'), ('Linux', 'aarch64', '6.6-microsoft-standard-WSL2'),
                 ('Linux', 'x86_64', '6.8.0-generic'), ('Windows', 'AMD64', '11')}
        for system, machine, release in sorted(hosts):
            with self.subTest(system=system, machine=machine), \
                    mock.patch.object(installer.platform, 'system', return_value=system), \
                    mock.patch.object(installer.platform, 'machine', return_value=machine), \
                    mock.patch.object(installer.platform, 'release', return_value=release):
                with self.assertRaises(ValueError) as refused:
                    installer.host()
                self.assertIn('WSL2', str(refused.exception))
                self.assertNotIn('macOS', str(refused.exception))
        with mock.patch.object(installer.platform, 'system', return_value='Linux'), \
                mock.patch.object(installer.platform, 'machine', return_value='x86_64'), \
                mock.patch.object(installer.platform, 'release',
                                  return_value='6.18.33.2-microsoft-standard-WSL2'):
            self.assertIsNone(installer.host())

    def test_installer_provisions_the_pinned_font_and_refuses_a_mismatch(self):
        """The font pin is enforced by hash, not by trusting the download.

        A stand-in extractor keeps the test independent of 7z: what is under
        test is the pin (archive hash, face hash, destination, idempotence) and
        the refusal, not py7zr itself.
        """
        import hashlib
        import importlib.util
        specification = importlib.util.spec_from_file_location(
            'uml_posix', REPO / 'scripts/uml_posix.py')
        installer = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(installer)

        face = self.root / 'face.ttf'
        face.write_bytes(b'pinned sarasa face')
        face_hash = hashlib.sha256(face.read_bytes()).hexdigest()
        archive = self.root / 'fixture.7z'
        archive.write_bytes(b'archive bytes')
        archive_hash = hashlib.sha256(archive.read_bytes()).hexdigest()
        extractor = self.root / 'extractor'
        # Shell sees $1=-I $2=-c $3=script $4=archive $5=target $6=face.
        extractor.write_text('#!/bin/sh\n'
                             'mkdir -p "$5"\n'
                             f'cp "{face}" "$5/SarasaMonoSC-Regular.ttf"\n'
                             'echo SarasaMonoSC-Regular.ttf\n')
        extractor.chmod(0o755)
        fonts = self.root / '.uml-viewer/fonts'
        with unittest.mock.patch.dict(os.environ, {
                'UML_VIEWER_FONT_URL': 'file://' + str(archive),
                'UML_VIEWER_FONT_ARCHIVE_SHA256': archive_hash,
                'UML_VIEWER_FONT_SHA256': face_hash,
                'UML_VIEWER_FONT_EXTRACTOR': str(extractor)}):
            installer.font(fonts)
            self.assertEqual(face.read_bytes(), (fonts / 'SarasaMonoSC-Regular.ttf').read_bytes())
            written = (fonts / 'SarasaMonoSC-Regular.ttf').stat().st_mtime_ns
            # A second pass must keep the already verified face untouched.
            installer.font(fonts)
            self.assertEqual(written, (fonts / 'SarasaMonoSC-Regular.ttf').stat().st_mtime_ns)
            # A pin that does not match the face must leave nothing behind.
            (fonts / 'SarasaMonoSC-Regular.ttf').unlink()
            with unittest.mock.patch.dict(os.environ, {'UML_VIEWER_FONT_SHA256': '0' * 64}):
                with self.assertRaises(ValueError) as caught:
                    installer.font(fonts)
                self.assertIn('SHA256 mismatch', str(caught.exception))
            # And a substituted archive never reaches extraction.
            with unittest.mock.patch.dict(os.environ, {'UML_VIEWER_FONT_ARCHIVE_SHA256': '0' * 64}):
                with self.assertRaises(ValueError) as caught:
                    installer.font(fonts)
                self.assertIn('archive SHA256 mismatch', str(caught.exception))
            self.assertFalse((fonts / 'SarasaMonoSC-Regular.ttf').exists())

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


    def stub_tool(self, tool, body='import sys\nraise SystemExit(3)\n'):
        directory = self.root / f'.uml-viewer/{tool}/.venv/bin'
        directory.mkdir(parents=True)
        python = directory / 'python'
        python.write_text('#!/usr/bin/python3\n' + body)
        python.chmod(0o755)

    def record(self):
        return json.loads((self.root / '.uml-viewer/quality-result.json').read_text())

    def test_record_matches_operation_canonical_root_and_actual_exit(self):
        # The companion rejects a record whose operation, projectRoot or exitCode
        # differs from the call it made; the published record must carry exactly
        # those three values, even through a symlinked project path and when
        # per-file mutation exits differ.
        self.stub_tool('mutator', 'import sys\nraise SystemExit(3 if sys.argv[-1].endswith("a.lua") else 0)\n')
        self.stub_tool('dryer', 'raise SystemExit(0)\n')
        (self.root / 'src/b.lua').write_text('return {}')
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {'sourceRoots': ['src']}}))
        alias = Path(tempfile.mkdtemp(prefix='uml link ')) / 'project'
        self.addCleanup(shutil.rmtree, alias.parent)
        alias.symlink_to(self.root, target_is_directory=True)
        cases = [('mutate', ['src/a.lua', 'src/b.lua'], 3), ('mutate', ['src/b.lua', 'src/a.lua'], 3),
                 ('mutate', ['src/b.lua'], 0), ('dry', [], 0)]
        for operation, args, expected in cases:
            with self.subTest(operation=operation, args=args):
                result = subprocess.run([str(alias / 'uml'), operation, *args], cwd=alias, env=self.env,
                                        text=True, capture_output=True, timeout=30)
                self.assertEqual(expected, result.returncode, result.stderr)
                record = self.record()
                self.assertEqual((operation, str(self.root.resolve()), result.returncode),
                                 (record['operation'], record['projectRoot'], record['exitCode']))

    def test_standard_crap_requests_the_whole_business_tree(self):
        # Ported from the retired PowerShell public-entry suite: the standard
        # craspper invocation keeps declared scope, options and caller arguments
        # in that order, and crapper's declared finding (exit 2) is valid.
        self.stub_tool('crapper', 'import json,sys\nfrom pathlib import Path\n'
                       'Path("crap-call.json").write_text(json.dumps(sys.argv[1:]))\nraise SystemExit(2)\n')
        (self.root / 'business tree').mkdir()
        (self.root / 'business tree/generated').mkdir()
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business tree'], 'exclude': ['business tree/generated'],
            'crapperOptions': ['--no-coverage']}}))
        result = self.entry('crap')
        self.assertEqual(2, result.returncode, result.stderr)
        self.assertEqual(['-m', 'crapper', '--full-tree', '--source-root', 'business tree',
                          '--exclude', 'business tree/generated', '--no-coverage'],
                         json.loads((self.root / 'crap-call.json').read_text()))
        self.assertEqual({'version': 1, 'operation': 'crap', 'exitCode': 2, 'validResult': True,
                          'crapScope': 'whole-tree', 'configured': False, 'tool': 'crapper',
                          'projectRoot': str(self.root.resolve())}, self.record())

    def test_standard_mutation_keeps_declared_scope_and_literal_argv(self):
        # Ported from the retired suite: declared exclusions filter the business
        # files while spaces and shell metacharacters reach mutator literally.
        self.stub_tool('mutator', 'import json,sys\nfrom pathlib import Path\n'
                       'p=Path("mutate-calls.jsonl")\n'
                       'with p.open("a") as f: f.write(json.dumps(sys.argv[1:])+"\\n")\n'
                       'raise SystemExit(3)\n')
        for name in ['business/keep.lua', 'business/generated/omit.lua', 'outside.lua']:
            file = self.root / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('return 1')
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business'], 'exclude': ['**/generated'],
            'mutatorOptions': ['--max-workers', '2']}}))
        result = self.entry('mutate', 'business/keep.lua', 'business/generated/omit.lua',
                            'outside.lua', 'business/keep again.lua')
        self.assertEqual(3, result.returncode, result.stderr)
        calls = [json.loads(line) for line in
                 (self.root / 'mutate-calls.jsonl').read_text().splitlines()]
        self.assertEqual([['-m', 'mutator', '--max-workers', '2', 'business/keep.lua']], calls)
        self.assertTrue(self.record()['validResult'])

    def test_standard_mutation_never_accepts_routing_overrides_from_the_caller(self):
        # Ported from the retired suite: scope belongs to the declaration, so a
        # caller cannot narrow or widen it, and no tool is launched.
        self.stub_tool('mutator')
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1,
            'standardTools': {'sourceRoots': ['src']}}))
        for flag in ('--source-root', '--root', '--exclude'):
            with self.subTest(flag=flag):
                result = self.entry('mutate', flag, 'elsewhere', 'src/a.lua')
                self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                self.assertIn('standardTools', result.stderr)
                self.assertFalse((self.root / '.uml-viewer/quality-result.json').exists())

    def test_invalid_standard_scope_never_launches_or_reuses_a_result(self):
        # Ported from the retired suite: unusable scope fails before the tool
        # runs and must not leave an older valid record behind.
        self.stub_tool('mutator')
        state = self.root / '.uml-viewer'
        for settings in ({'sourceRoots': ['missing']}, {'sourceRoots': ['..']},
                         {'mutatorOptions': ['--mutate-all']},
                         {'exclude': ['../outside']}):
            with self.subTest(settings=settings):
                (state / 'quality-result.json').write_text('{"validResult":true}')
                (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1,
                    'standardTools': settings}))
                result = self.entry('mutate')
                self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                self.assertFalse((state / 'quality-result.json').exists())

    def test_accessory_only_manifest_is_rejected_without_launching(self):
        # Ported from the retired suite: aliasResultExitCodes died with the JVM
        # implementation, so a manifest declaring only it cannot dispatch.
        self.stub_tool('mutator')
        (self.root / 'uml-quality.json').write_text(json.dumps({'version': 1,
            'aliasResultExitCodes': {'mutate': [0, 3]}}))
        (self.root / '.uml-viewer/quality-result.json').write_text('{"validResult":true}')
        result = self.entry('mutate', 'src/a.lua')
        self.assertEqual(1, result.returncode, result.stdout + result.stderr)
        self.assertIn('Clojure alias 已移除', result.stderr)
        self.assertFalse((self.root / '.uml-viewer/quality-result.json').exists())

    def test_love_and_unknown_runtime_values_are_refused(self):
        # #144: Rust is the only implementation. `love` is refused the same
        # way `jvm` was: nothing is dispatched and the message names rust.
        for args in (('--runtime',), ('--runtime', 'invalid'), ('--runtime', 'jvm'),
                     ('--runtime', 'love'), ('--runtime', 'love', 'ir'),
                     ('--runtime', 'rust', 'ir', '--runtime', 'x')):
            with self.subTest(args=args):
                result = self.entry(*args)
                self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                self.assertIn('--runtime', result.stderr)
                self.assertIn('rust', result.stderr)
                self.assertIn('已移除', result.stderr)
                self.assertIsNone(self.launcher_record())

    def test_default_runtime_dispatches_headless_subcommands_to_the_rust_binary(self):
        # #144: no --runtime flag selects the Rust binary.
        record, env = self.rust_stub()
        arguments = ['ir', 'policy.edn', '结果 图.edn', 'literal;$value']
        result = subprocess.run([str(self.root / 'uml'), *arguments], cwd=self.root,
                                env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(7, result.returncode, result.stderr)
        self.assertEqual(arguments, record.read_text().splitlines())

    def test_default_runtime_launches_the_rust_viewer_and_records_the_runtime(self):
        record, env = self.rust_stub()
        result = subprocess.run([str(self.root / 'uml'), 'diagram with spaces.edn'],
                                cwd=self.root, env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(0, result.returncode, result.stderr)
        launcher = self.launcher_record()
        self.addCleanup(self.stop_stub, launcher)
        self.assertEqual('rust', launcher['runtime'])
        self.assertEqual(['viewer', '--backend', 'herdr', 'diagram with spaces.edn'], launcher['argv'])
        observed = Path(f'/proc/{launcher["pid"]}/stat').read_text().rsplit(')', 1)[-1].split()[19]
        self.assertEqual(observed, launcher['started'])

    def test_default_runtime_without_a_built_binary_names_the_build_step(self):
        env = dict(self.env, UML_VIEWER_RUST_EXECUTABLE=str(self.root / 'missing'))
        result = subprocess.run([str(self.root / 'uml'), 'ir'], cwd=self.root,
                                env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(1, result.returncode)
        self.assertIn('cargo build --release --locked', result.stderr)

    def test_default_runtime_reaches_the_real_binary(self):
        binary = REPO / 'target/release/uml-viewer'
        if not binary.is_file():
            self.skipTest('run cargo build --release --locked first')
        env = dict(self.env)
        env.pop('UML_VIEWER_RUST_EXECUTABLE', None)
        result = subprocess.run([str(self.root / 'uml'), 'scene'], cwd=self.root,
                                env=env, capture_output=True, text=True, timeout=30)
        # #137: the ported `scene` role answers its own usage error.
        self.assertEqual(2, result.returncode, result.stderr)
        self.assertIn('uml-viewer scene', result.stderr)

    def test_rust_ir_matches_all_four_language_goldens(self):
        # #136: the ported ir role reproduces the JVM goldens structurally
        # for every fixture project through the public entry.
        binary = REPO / 'target/release/uml-viewer'
        if not binary.is_file():
            self.skipTest('run cargo build --release --locked first')
        env = dict(self.env)
        env.pop('UML_VIEWER_RUST_EXECUTABLE', None)
        for language in ('lua', 'typescript', 'rust', 'python'):
            with self.subTest(language=language):
                output = self.root / f'{language}.golden-result.edn'
                result = subprocess.run(
                    [str(self.root / 'uml'), 'ir',
                     '--root', str(REPO / f'{language}-fixture'),
                     '--policy', 'policy.edn', '--output', str(output)],
                    cwd=self.root, env=env, capture_output=True, text=True, timeout=30)
                self.assertEqual(0, result.returncode, result.stderr)
                comparison = subprocess.run(
                    [sys.executable, '-I', str(REPO / 'scripts/compare_ir.py'), str(output),
                     str(REPO / 'fixtures/jvm-goldens' / f'{language}.ir.edn')],
                    cwd=REPO, capture_output=True, text=True)
                self.assertEqual(0, comparison.returncode, comparison.stdout + comparison.stderr)

    def test_rust_ir_runs_on_this_repository(self):
        # #136/#144: the default runtime generates IR for this repository's
        # own Rust source tree.
        binary = REPO / 'target/release/uml-viewer'
        if not binary.is_file():
            self.skipTest('run cargo build --release --locked first')
        env = dict(self.env)
        env.pop('UML_VIEWER_RUST_EXECUTABLE', None)
        output = self.root / 'repo.edn'
        result = subprocess.run(
            [str(self.root / 'uml'), 'ir', '--root', str(REPO),
             '--policy', 'examples/uml-viewer.policy.edn', '--output', str(output)],
            cwd=self.root, env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(0, result.returncode, result.stderr)
        document = output.read_text()
        self.assertIn(':lang :rust', document)
        self.assertIn('"crates/edn/src/lib.rs"', document)

    def rust_stub(self, preflight='herdr'):
        """A stand-in Rust binary: records argv, answers the preflight."""
        stub = self.root / 'rust stub'
        record = self.root / 'rust-argv.txt'
        stub.write_text(
            '#!/bin/sh\n'
            'if [ "$1" = companion-preflight ]; then\n'
            f'  printf "%s\\n" {preflight!r}\n'
            '  exit 0\n'
            'fi\n'
            f'printf "%s\\n" "$@" > {str(record)!r}\n'
            'if [ "$1" = viewer ]; then sleep 30; fi\n'
            'exit 7\n')
        stub.chmod(0o755)
        return record, dict(self.env, UML_VIEWER_RUST_EXECUTABLE=str(stub))

    def launcher_record(self):
        path = self.root / '.uml-viewer/viewer-process.json'
        return json.loads(path.read_text()) if path.exists() else None

    def stop_stub(self, record):
        if not record:
            return
        pid = record['pid']
        try:
            Path(f'/proc/{pid}').exists() and os.kill(pid, 15)
        except (ProcessLookupError, PermissionError):
            pass

    def test_preflight_failure_stops_launch_without_a_viewer(self):
        # Ported from the retired suite: an unusable preflight answer stops the
        # launch before any viewer, launcher record or ownership exists.
        for preflight in ('', 'tmux', 'herdr\nextra'):
            with self.subTest(preflight=preflight):
                _, env = self.rust_stub(preflight=preflight)
                result = subprocess.run([str(self.root / 'uml'), 'diagram.edn'], cwd=self.root,
                                        env=env, capture_output=True, text=True, timeout=30)
                self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                self.assertIn('preflight', result.stderr)
                self.assertIsNone(self.launcher_record())
                self.assertFalse((self.root / '.uml-viewer/companion.edn').exists())

    def test_launch_records_a_real_identity_and_forwards_literal_argv(self):
        # The launcher record must carry the viewer's own creation identity,
        # and argv reaches the Rust binary unchanged except for the backend the
        # preflight consumed.
        record, env = self.rust_stub()
        arguments = ['diagram with spaces.edn', 'literal;$value']
        result = subprocess.run([str(self.root / 'uml'), *arguments], cwd=self.root,
                                env=env, capture_output=True, text=True, timeout=30)
        self.assertEqual(0, result.returncode, result.stderr)
        launcher = self.launcher_record()
        self.addCleanup(self.stop_stub, launcher)
        self.assertIsNotNone(launcher, result.stdout)
        self.assertEqual('rust', launcher['runtime'])
        observed = Path(f'/proc/{launcher["pid"]}/stat').read_text().rsplit(')', 1)[-1].split()[19]
        self.assertEqual(observed, launcher['started'])
        self.assertEqual(['viewer', '--backend', 'herdr', *arguments], launcher['argv'])
        # The stub writes after the viewer exits; the preflight consumes the
        # only --backend pair, so the viewer sees exactly the forwarded argv.

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
        cargo, _ = self.cargo_stub()
        base = dict(self.env, UML_VIEWER_TOOL_ROOT='', UML_VIEWER_REPO_URL=str(source),
                    UML_CARGO_EXECUTABLE=str(cargo),
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
        env = dict(self.env, UML_CARGO_EXECUTABLE='/missing/cargo')
        result = subprocess.run([entry, '--install-only'], cwd=self.root, env=env, capture_output=True, text=True)
        self.assertEqual(1, result.returncode)
        self.assertIn('cargo', result.stderr)
        self.assertFalse((self.root / '.uml-viewer').exists())

    def test_installer_no_longer_provisions_love_lua_or_cimgui(self):
        # #144: the Lua runtime, its busted/luacov toolchain and the cimgui
        # binding are gone, so no precheck or provisioning function remains.
        installer = self.installer_module()
        for name in ('love_command', 'lua_command', 'cimgui'):
            self.assertFalse(hasattr(installer, name), name)
        self.assertFalse(hasattr(installer, 'CIMGUI_URL'))
        self.assertFalse(hasattr(installer, 'LUA_PROBE'))

    def installer_module(self):
        import importlib.util
        specification = importlib.util.spec_from_file_location(
            'uml_posix', REPO / 'scripts/uml_posix.py')
        installer = importlib.util.module_from_spec(specification)
        specification.loader.exec_module(installer)
        return installer

    def cargo_stub(self, version='1.99.1'):
        """A stand-in cargo: answers --version, records builds, emits a binary."""
        stub = self.root / 'cargo stub'
        record = self.root / 'cargo-calls.txt'
        stub.write_text(
            '#!/bin/sh\n'
            f'if [ "$1" = --version ]; then printf "cargo {version} (stub)\\n"; exit 0; fi\n'
            f'printf "%s|%s\\n" "$(pwd -P)" "$*" >> {str(record)!r}\n'
            'mkdir -p target/release\n'
            'printf "#!/bin/sh\\n" > target/release/uml-viewer\n'
            'chmod +x target/release/uml-viewer\n')
        stub.chmod(0o755)
        return stub, record

    def test_installer_preflight_names_a_missing_cargo(self):
        # #143: the installer preflights the Rust toolchain; a missing cargo
        # names the fix instead of surfacing an OS error mid-install.
        installer = self.installer_module()
        env = dict(os.environ)
        env.pop('UML_CARGO_EXECUTABLE', None)
        with unittest.mock.patch.dict(os.environ, env, clear=True), \
                unittest.mock.patch.object(installer.shutil, 'which', return_value=None):
            with self.assertRaises(ValueError) as caught:
                installer.cargo_command()
        message = str(caught.exception)
        self.assertIn('cargo', message)
        self.assertIn('rustup', message)

    def test_installer_preflight_requires_git(self):
        installer = self.installer_module()
        with unittest.mock.patch.object(installer.shutil, 'which', return_value=None):
            with self.assertRaises(ValueError) as caught:
                installer.git_command()
        self.assertIn('git', str(caught.exception))

    def test_rust_toolchain_channel_comes_from_the_toolchain_file(self):
        installer = self.installer_module()
        checkout = self.root / 'checkout'
        checkout.mkdir()
        (checkout / 'rust-toolchain.toml').write_text('[toolchain]\nchannel = "1.99.1"\n')
        self.assertEqual('1.99.1', installer.rust_channel(checkout))
        (checkout / 'rust-toolchain.toml').unlink()
        with self.assertRaises(ValueError) as caught:
            installer.rust_channel(checkout)
        self.assertIn('rust-toolchain.toml', str(caught.exception))

    def test_installer_rejects_a_cargo_mismatched_with_the_toolchain_pin(self):
        installer = self.installer_module()
        checkout = self.root / 'checkout'
        checkout.mkdir()
        (checkout / 'rust-toolchain.toml').write_text('[toolchain]\nchannel = "1.99.1"\n')
        cargo, _ = self.cargo_stub(version='1.98.0')
        with self.assertRaises(ValueError) as caught:
            installer.check_rust_toolchain(str(cargo), checkout)
        message = str(caught.exception)
        self.assertIn('1.99.1', message)
        self.assertIn('1.98.0', message)
        (checkout / 'rust-toolchain.toml').write_text('[toolchain]\nchannel = "1.98.0"\n')
        self.assertEqual('1.98.0', installer.check_rust_toolchain(str(cargo), checkout))

    def test_rust_build_runs_the_locked_release_build(self):
        installer = self.installer_module()
        checkout = self.root / 'checkout'
        checkout.mkdir()
        (checkout / 'rust-toolchain.toml').write_text('[toolchain]\nchannel = "1.99.1"\n')
        cargo, record = self.cargo_stub()
        binary = installer.rust_build(str(cargo), checkout)
        self.assertEqual(['build --release --locked'],
                         [line.rsplit('|', 1)[-1] for line in record.read_text().splitlines()])
        self.assertIn(str(checkout), record.read_text())
        self.assertEqual(checkout / 'target/release/uml-viewer', binary)
        self.assertTrue(os.access(binary, os.X_OK))
        # A build that produces no binary is an installer failure, not a pass.
        quiet = self.root / 'cargo quiet'
        quiet.write_text('#!/bin/sh\n'
                         'if [ "$1" = --version ]; then printf "cargo 1.99.1 (stub)\\n"; exit 0; fi\n'
                         'exit 0\n')
        quiet.chmod(0o755)
        (checkout / 'target/release/uml-viewer').unlink()
        with self.assertRaises(ValueError) as caught:
            installer.rust_build(str(quiet), checkout)
        self.assertIn('uml-viewer', str(caught.exception))

    def test_x11_library_absence_is_a_warning_not_a_requirement(self):
        # #139 chose the Wayland-undecorated window because libxkbcommon-x11
        # is absent on the delivery host; the installer only warns (#143).
        installer = self.installer_module()
        self.assertEqual([], installer.x11_warnings(present=True))
        warnings = installer.x11_warnings(present=False)
        self.assertTrue(warnings)
        self.assertIn('libxkbcommon-x11', warnings[0])
        self.assertIn('Wayland', warnings[0])

    def test_install_names_a_missing_cargo_before_touching_state(self):
        entry = str(REPO / 'scripts/get-uml-viewer')
        env = dict(self.env, UML_CARGO_EXECUTABLE='/missing/cargo')
        result = subprocess.run([entry, '--install-only'], cwd=self.root, env=env,
                                capture_output=True, text=True)
        self.assertEqual(1, result.returncode)
        self.assertIn('cargo', result.stderr)
        self.assertIn('rustup', result.stderr)
        self.assertFalse((self.root / '.uml-viewer').exists())

class ViewerLaunchEnv(unittest.TestCase):
    """#139/#144: the launcher sets the WSLg render defaults for the only
    runtime without overriding user values."""

    @classmethod
    def setUpClass(cls):
        # check.py runs this file with python -I: import by directory.
        sys.path.insert(0, str(Path(__file__).resolve().parent))

    def test_rust_runtime_gets_render_defaults(self):
        from uml_launch import viewer_env
        env = viewer_env({})
        self.assertEqual('d3d12', env['GALLIUM_DRIVER'])
        self.assertEqual('gl', env['WGPU_BACKEND'])

    def test_user_values_win(self):
        from uml_launch import viewer_env
        env = viewer_env({'GALLIUM_DRIVER': 'virgl'})
        self.assertEqual('virgl', env['GALLIUM_DRIVER'])
        self.assertEqual('gl', env['WGPU_BACKEND'])

if __name__ == '__main__':
    unittest.main()
