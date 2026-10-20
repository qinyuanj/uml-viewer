#!/usr/bin/env python3
"""在原生 LOVE 里运行一个既有 GUI 行为 driver，断言可观察结果。

既有的 `scripts/test_love_*.lua` driver 都是平台无关的：它们通过
`UML_VIEWER_TEST_DRIVER` 驱动真实 LOVE 回调并写入证据文件。原先每个 driver
配一个 PowerShell 包装器（设环境变量、启动 love、检查证据）；本脚本用同一
个 POSIX harness 取代那些包装器，因此断言本身不丢失。

用法：
    python3 -I scripts/run_love_driver.py navigation
    python3 -I scripts/run_love_driver.py navigation --evidence DIR --seconds 60
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]


def wait_for(test, timeout, interval=0.05, message='condition'):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = test()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f'timed out waiting for {message}')


# What each driver proves, expressed as the evidence files it must publish.
# These lists were taken from the PowerShell wrappers they replace, so retiring
# a wrapper does not silently drop a behavioural assertion.
DRIVERS = {
    'class_boxes': {'seconds': 20, 'evidence': ['class-boxes-pass.txt', 'render.png']},
    'detail_scroll': {'seconds': 30, 'evidence': ['inspector-pass.txt', 'inspector.png', 'inspector-empty.png']},
    'inspector': {'seconds': 30, 'evidence': ['inspector-pass.txt', 'inspector.png']},
    'navigation': {'seconds': 20, 'evidence': ['navigation-pass.txt', 'navigation-entered.edn',
                                               'navigation-zoomed.edn', 'navigation-panned.edn',
                                               'navigation-fitted.edn', 'navigation-back.edn',
                                               'navigation-selected.edn', 'navigation-wheel.edn',
                                               'render.png']},
    'search': {'seconds': 30, 'evidence': ['search-pass.txt', 'search.png']},
    'source_jump': {'seconds': 30, 'evidence': ['source-jump-pass.txt', 'module.txt', 'module.png',
                                                'member.txt', 'member.png']},
    'source_window': {'seconds': 20, 'evidence': ['source-window.txt', 'source-window.png']},
    'review_fixes': {'seconds': 30, 'evidence': []},
    'mail': {'seconds': 20, 'evidence': []},
    'metrics': {'seconds': 20, 'evidence': ['render.png', 'scene-1.edn', 'scene-2.edn']},
    'fonts': {'seconds': 30, 'evidence': []},
    'font_parity': {'seconds': 30, 'evidence': []},
    'dpi_window': {'seconds': 30, 'evidence': []},
    'companion_worker': {'seconds': 90, 'evidence': ['worker-ui-pass.txt']},
}


def love_executable():
    love = shutil.which(os.environ.get('UML_LOVE_EXECUTABLE', 'love'))
    if not love:
        raise ValueError('LOVE 11.5 is required: install love or set UML_LOVE_EXECUTABLE')
    return love


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('driver', help='driver name, e.g. navigation for test_love_navigation.lua')
    parser.add_argument('--evidence')
    parser.add_argument('--seconds', type=float, default=None)
    parser.add_argument('--project-root', help='use this project instead of an isolated copy')
    parser.add_argument('--diagram', help='initial diagram, relative to the project root')
    parser.add_argument('--pass-marker', action='append', default=[],
                        help='evidence file the driver must create (repeatable)')
    arguments = parser.parse_args()

    spec = DRIVERS.get(arguments.driver, {'seconds': 90, 'evidence': []})
    seconds = arguments.seconds or spec['seconds']
    markers = list(arguments.pass_marker) or list(spec['evidence'])
    driver = REPO / f'scripts/test_love_{arguments.driver}.lua'
    if not driver.is_file():
        print(f'no driver: {driver}', file=sys.stderr)
        return 2
    evidence = Path(arguments.evidence or tempfile.mkdtemp(prefix='uml-love-evidence-'))
    evidence.mkdir(parents=True, exist_ok=True)
    temporary_project = arguments.project_root is None
    project = Path(arguments.project_root) if arguments.project_root else \
        Path(tempfile.mkdtemp(prefix='uml love 项目 '))
    runtime = Path(tempfile.mkdtemp(prefix='uml-love-xdg-'))
    env = dict(os.environ,
               XDG_CONFIG_HOME=str(runtime / 'config'), XDG_DATA_HOME=str(runtime / 'data'),
               XDG_CACHE_HOME=str(runtime / 'cache'), XDG_STATE_HOME=str(runtime / 'state'),
               UML_VIEWER_PROJECT_ROOT=str(project), UML_VIEWER_EVIDENCE=str(evidence),
               UML_VIEWER_TEST_SECONDS=str(seconds),
               UML_VIEWER_TEST_DRIVER=str(driver),
               UML_MAIL_TEST_ROOT=str(project), UML_COMPANION_NO_ATTACH='1')
    env.pop('UML_CIMGUI_DIRECTORY', None)
    result = {'driver': arguments.driver, 'project': str(project), 'evidence': str(evidence)}
    try:
        if temporary_project:
            (project / '.uml-viewer').mkdir(parents=True)
            if (REPO / '.uml-viewer/cimgui-love-1.90-1').exists():
                shutil.copytree(REPO / '.uml-viewer/cimgui-love-1.90-1',
                                project / '.uml-viewer/cimgui-love-1.90-1')
            shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
        diagram = arguments.diagram or 'diagram.edn'
        process = subprocess.run([love_executable(), str(REPO / 'love'), 'viewer', diagram],
                                 cwd=project, env=env, capture_output=True, text=True,
                                 timeout=seconds + 120)
        (evidence / 'stdout.txt').write_text(process.stdout or '')
        (evidence / 'stderr.txt').write_text(process.stderr or '')
        for marker in markers:
            assert (evidence / marker).exists() or (project / marker).exists(), \
                f'driver did not publish {marker}'
        assert process.returncode == 0, \
            f'viewer exited {process.returncode}: {(process.stderr or "").strip()[-800:]}'
        result['ok'] = True
    except (AssertionError, subprocess.TimeoutExpired) as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        if result.get('ok') and temporary_project:
            shutil.rmtree(project, ignore_errors=True)
        elif not result.get('ok'):
            result['project_retained'] = str(project)
        shutil.rmtree(runtime, ignore_errors=True)
        (evidence / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    if result.get('ok'):
        print(f'PASS driver {arguments.driver}; evidence: {evidence}')
        return 0
    print(f'FAIL driver {arguments.driver}: {result.get("failure")}', file=sys.stderr)
    print(f'evidence: {evidence}', file=sys.stderr)
    return 1


if __name__ == '__main__':
    sys.exit(main())
