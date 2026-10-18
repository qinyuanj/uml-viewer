"""Native public-entry checks for the installed standard quality tools."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest

REPO = Path(__file__).resolve().parents[1]


class StandardQuality(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(prefix='quality project with spaces ')
        self.addCleanup(self.tmp.cleanup)
        self.project = Path(self.tmp.name)
        installed = self.project / '.uml-viewer/uml-viewer/scripts'
        installed.mkdir(parents=True)
        for script in REPO.joinpath('scripts').glob('*.ps1'):
            shutil.copy2(script, installed)
        shutil.copy2(REPO / 'scripts/uml.ps1', self.project / 'uml.ps1')

    def run_quality(self, operation, *argv):
        return subprocess.run(['pwsh', '-NoProfile', '-File', str(self.project / 'uml.ps1'),
                               operation, *argv], cwd=REPO, text=True, capture_output=True)

    def fixture_tool(self, tool):
        directory = self.project / '.uml-viewer' / tool
        if os.name == 'nt':
            native = directory / '.venv/Scripts'
            directory.mkdir(parents=True)
            subprocess.run([sys.executable, '-m', 'venv', '--without-pip', str(directory / '.venv')],
                           check=True, capture_output=True)
            package = self.project / tool
            package.mkdir()
            (package / '__init__.py').write_text('')
            (package / '__main__.py').write_text(
                'import json, os, sys\n'
                'print(json.dumps({"cwd": os.getcwd(), "argv": sys.argv[1:]}))\n'
                'print("standard diagnostic", file=sys.stderr)\n'
                'sys.exit(3)\n')
        else:
            directory.mkdir(parents=True)
            entry = directory / tool
            entry.write_text('#!/usr/bin/env python3\nimport json, os, sys\n'
                             'print(json.dumps({"cwd": os.getcwd(), "argv": sys.argv[1:]}))\n'
                             'print("standard diagnostic", file=sys.stderr)\nsys.exit(3)\n')
            entry.chmod(0o755)

    def test_standard_crap_requests_complete_snapshot_with_scope_arguments(self):
        self.fixture_tool('crapper')
        result = self.run_quality('crap', '--source-root', 'business tree', '--exclude', 'business tree/generated')
        self.assertEqual(3, result.returncode, result.stderr)
        self.assertEqual({'cwd': str(self.project), 'argv': ['--full-tree', '--source-root',
                         'business tree', '--exclude', 'business tree/generated']}, json.loads(result.stdout))
        self.assertIn('standard diagnostic', result.stderr)

    def test_standard_only_manifest_persists_scope_without_command_override(self):
        self.fixture_tool('crapper')
        (self.project / 'business tree').mkdir()
        (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business tree'], 'exclude': ['business tree/generated'],
            'crapperOptions': ['--no-coverage']}}))
        result = self.run_quality('crap')
        self.assertEqual(3, result.returncode, result.stdout + result.stderr)
        self.assertEqual(['--full-tree', '--source-root', 'business tree', '--exclude',
                         'business tree/generated', '--no-coverage'], json.loads(result.stdout)['argv'])

    def test_standard_mutation_filters_changed_business_files_without_forcing_rerun(self):
        self.fixture_tool('mutator')
        for name in ['business/keep.lua', 'business/generated/omit.lua', 'outside.lua']:
            file = self.project / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('return 1')
        (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business'], 'exclude': ['**/generated'], 'mutatorOptions': ['--max-workers', '2']}}))
        result = self.run_quality('mutate', 'business/keep.lua', 'business/generated/omit.lua', 'outside.lua')
        self.assertEqual(3, result.returncode, result.stdout + result.stderr)
        self.assertEqual(['--max-workers', '2', 'business/keep.lua'],
                         json.loads(result.stdout)['argv'])
        self.assertIn('outside.lua', result.stderr)
        empty = self.run_quality('mutate', 'business/generated/omit.lua')
        self.assertEqual(1, empty.returncode, empty.stdout + empty.stderr)
        self.assertEqual('', empty.stdout, 'empty selection launched the tool')

    def test_standard_mutation_accepts_crapper_dot_prefix_exclusion(self):
        self.fixture_tool('mutator')
        for name in ['business/keep.lua', 'business/generated/omit.lua']:
            file = self.project / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('return 1')
        (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business'], 'exclude': ['./business/generated/']}}))
        result = self.run_quality('mutate', 'business')
        self.assertEqual(3, result.returncode, result.stdout + result.stderr)
        self.assertEqual(['business/keep.lua'], json.loads(result.stdout)['argv'])

    def test_standard_mutation_embedded_double_star_stays_in_one_segment(self):
        self.fixture_tool('mutator')
        for name in ['business/keep.lua', 'business/generated/omit.lua',
                     'business/generated/deep/keep.lua']:
            file = self.project / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('return 1')
        (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business'], 'exclude': ['business/gen**.lua']}}))
        result = self.run_quality('mutate', 'business')
        self.assertEqual(3, result.returncode, result.stdout + result.stderr)
        self.assertEqual({'business/keep.lua', 'business/generated/omit.lua',
                          'business/generated/deep/keep.lua'}, set(json.loads(result.stdout)['argv']))

    def test_invalid_standard_scope_cannot_launch_or_reuse_old_result(self):
        self.fixture_tool('mutator')
        record = self.project / '.uml-viewer/quality-result.json'
        for settings in [{'sourceRoots': ['missing']}, {'sourceRoots': ['..']},
                         {'mutatorOptions': ['--mutate-all']}, {'exclude': ['../outside']}]:
            (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': settings}))
            record.write_text('{"validResult":true}')
            result = self.run_quality('mutate')
            self.assertEqual(1, result.returncode, result.stdout + result.stderr)
            self.assertEqual('', result.stdout)
            self.assertFalse(record.exists())

    def test_operation_override_still_validates_all_standard_scope_before_launch(self):
        command = self.project / 'configured.ps1'
        command.write_text('Write-Output "configured command launched"; exit 0\n')
        operations = {name: {'argv': [str(command)], 'forwardArgs': 'none',
                            'validResultExitCodes': [0]} for name in ['crap', 'mutate']}
        operations['crap']['scope'] = 'whole-tree'
        record = self.project / '.uml-viewer/quality-result.json'
        for settings in [{'sourceRoots': ['missing']}, {'sourceRoots': ['..']},
                         {'exclude': ['../outside']}]:
            with self.subTest(settings=settings):
                (self.project / 'uml-quality.json').write_text(json.dumps({
                    'version': 1, 'standardTools': settings, 'operations': operations}))
                record.write_text('{"validResult":true}')
                result = self.run_quality('mutate')
                self.assertEqual(1, result.returncode, result.stdout + result.stderr)
                self.assertEqual('', result.stdout, 'invalid config launched override')
                self.assertFalse(record.exists())

    def test_real_mutator_scan_obeys_filtered_targets_instead_of_reexpanding_roots(self):
        sibling = REPO
        while sibling.name != 'uml-viewer' and sibling != sibling.parent:
            sibling = sibling.parent
        tool = sibling.parent / 'mutator'
        crapper = sibling.parent / 'crapper'
        if os.name != 'nt' or not (tool / '.venv/Lib/site-packages').is_dir():
            self.skipTest('native installed sibling Mutator dependencies unavailable')
        self.fixture_tool('mutator')
        shutil.rmtree(self.project / 'mutator')
        shutil.copytree(tool / 'src/mutator', self.project / 'mutator', ignore=shutil.ignore_patterns('__pycache__'))
        shutil.copytree(crapper / 'src/crapper', self.project / 'crapper', ignore=shutil.ignore_patterns('__pycache__'))
        site = self.project / '.uml-viewer/mutator/.venv/Lib/site-packages'
        site.mkdir(parents=True, exist_ok=True)
        (site / 'readonly-dependencies.pth').write_text(str(tool / '.venv/Lib/site-packages'))
        for name in ['business/keep.lua', 'business/generated/omit.lua']:
            file = self.project / name
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('local M = {}\nfunction M.value() return 1 + 2 end\nreturn M\n')
        (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business'], 'exclude': ['business/generated']}}))
        result = self.run_quality('mutate', '--scan', '--no-coverage', 'business')
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        self.assertIn('keep.lua', result.stdout)
        self.assertNotIn('omit.lua', result.stdout)

    def test_alias_only_manifest_is_rejected_without_launch(self):
        # The Clojure alias route was deleted with the JVM implementation.
        (self.project / 'uml-quality.json').write_text(
            json.dumps({'version': 1, 'aliasResultExitCodes': {'mutate': [0, 3]}}))
        record = self.project / '.uml-viewer/quality-result.json'
        record.write_text('{"validResult":true}')
        result = self.run_quality('mutate')
        self.assertEqual(1, result.returncode, result.stdout + result.stderr)
        self.assertIn('aliasResultExitCodes', result.stderr)
        self.assertFalse(record.exists())

    def test_standard_mutation_preserves_differential_files_output_and_findings_status(self):
        self.fixture_tool('mutator')
        result = self.run_quality('mutate', 'business tree/file with spaces.lua', 'literal;$value')
        self.assertEqual(3, result.returncode, result.stderr)
        self.assertEqual({'cwd': str(self.project), 'argv': ['business tree/file with spaces.lua',
                         'literal;$value']}, json.loads(result.stdout))
        self.assertIn('standard diagnostic', result.stderr)

    def test_real_crapper_refresh_keeps_all_business_classes_and_excludes_generated_code(self):
        sibling = REPO
        while sibling.name != 'uml-viewer' and sibling != sibling.parent:
            sibling = sibling.parent
        source = sibling.parent / 'crapper/src/crapper'
        if not source.is_dir():
            self.skipTest('read-only sibling crapper checkout unavailable')
        self.fixture_tool('crapper')
        if os.name == 'nt':
            dependencies = sibling.parent / 'crapper/.venv/Lib/site-packages'
            if not dependencies.is_dir():
                self.skipTest('installed sibling crapper dependencies unavailable')
            site = self.project / '.uml-viewer/crapper/.venv/Lib/site-packages'
            site.mkdir(parents=True, exist_ok=True)
            (site / 'readonly-contract-dependencies.pth').write_text(str(dependencies))
        shutil.rmtree(self.project / 'crapper')
        shutil.copytree(source, self.project / 'crapper', ignore=shutil.ignore_patterns('__pycache__'))
        for path in ['business tree/first.lua', 'business tree/second.lua',
                     'business tree/generated/omit.lua', 'outside.lua']:
            file = self.project / path
            file.parent.mkdir(parents=True, exist_ok=True)
            file.write_text('local M = {}\nfunction M.value() return 1 end\nreturn M\n')
        (self.project / 'uml-quality.json').write_text(json.dumps({'version': 1, 'standardTools': {
            'sourceRoots': ['business tree'], 'exclude': ['business tree/generated'],
            'crapperOptions': ['--no-coverage']}}))
        result = self.run_quality('crap')
        self.assertEqual(0, result.returncode, result.stdout + result.stderr)
        snapshot = self.project / '.metrics/crap.edn'
        contents = snapshot.read_text()
        self.assertIn('first', contents)
        self.assertIn('second', contents)
        self.assertNotIn('omit', contents)
        self.assertNotIn('outside', contents)
        before = snapshot.read_bytes()
        partial = self.run_quality('crap', '--no-coverage', 'business tree/first.lua')
        self.assertEqual(1, partial.returncode, partial.stdout + partial.stderr)
        self.assertEqual(before, snapshot.read_bytes(), 'partial refresh discarded another class')


if __name__ == '__main__':
    unittest.main()
