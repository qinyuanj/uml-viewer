#!/usr/bin/env python3
"""在真实 viewer 里运行一个 GUI 行为 driver，断言可观察结果。

driver 有声明式 EDN 脚本（`scripts/drivers/<name>.edn`，由 #139 的 egui
viewer 重放）时用 Rust viewer；否则沿用 `scripts/test_love_*.lua` 在原生
LOVE 里重放（两实现并存到 #144）。两类 driver 都通过
`UML_VIEWER_TEST_DRIVER` 拿脚本路径、写入证据文件，并遵守同样的退出契约。
原先每个 driver 配一个包装器（设环境变量、启动 viewer、检查证据）；那些
PowerShell 包装器已随 Windows 原生支持退役（ADR 0007），本脚本是它们唯一的
POSIX 替代，因此断言本身不丢失。

用法：
    python3 -I scripts/run_viewer_driver.py metrics
    python3 -I scripts/run_viewer_driver.py navigation --runtime love
    python3 -I scripts/run_viewer_driver.py metrics --evidence DIR --seconds 60
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
# These lists came from the retired PowerShell wrappers, so retiring a wrapper
# did not silently drop a behavioural assertion.
# Each driver asserts against specific project content; the retired PowerShell
# wrappers wrote that content before launching LOVE. These fixtures replace
# exactly that setup, so retiring a wrapper dropped no behavioural assertion.
def prepare_metrics(project):
    (project / '.metrics').mkdir(parents=True, exist_ok=True)
    (project / 'diagram.edn').write_text(
        '{:packages [{:label "Adapters" :classes [{:id :ffi :ns "ffi"}'
        ' {:id :imgui :ns "imgui"}]}] :edges [{:from :ffi :to :imgui}]}')
    (project / '.metrics/crap.edn').write_text(
        '{:entries [{:namespace "ffi" :name "draw" :complexity 1'
        ' :coverage 100 :crap 1}]}')


def prepare_navigation(project):
    children = ' '.join(f'{{:id :source.child{n}}}' for n in range(1, 21))
    ops = ' '.join(f'"operation{n}"' for n in range(1, 71))
    (project / 'diagram.edn').write_text(
        '{:hierarchical true :title "Navigation acceptance" :order [:source :other]'
        f' :classes [{{:id :source}} {{:id :source.lua}} {{:id :source.lua.parser}} {children}'
        f' {{:id :other :ops [{ops}]}}] :edges []}}')


def prepare_search(project):
    (project / 'diagram.edn').write_text(
        '{:hierarchical true :classes [{:id :alpha.deep.parser :name "解析器"'
        ' :ns "love.alpha.deep.parser"} {:id :beta.parser :name "解析器"'
        ' :ns "love.beta.parser"}] :edges [{:from :alpha.deep.parser'
        ' :to :beta.parser :kind :dependency}]}')


def prepare_review_fixes(project):
    (project / 'diagram.edn').write_text('{:packages [{:id :p :classes [{:id :a}]}]}')


def prepare_source_jump(project):
    # The retired wrapper named the source file absolutely; the driver asserts
    # the row it lands on, so the path must resolve exactly as written.
    source = project / 'source.lua'
    source.write_text("local M = {}\n" + ''.join(f'-- preceding line {n}\n' for n in range(1, 81))
                      + "\tfunction M.target() local 变量 = 'target ## %'; return 变量 end\nreturn M\n")
    (project / 'diagram.edn').write_text(
        '{:hierarchical true :title "Source jump acceptance" :classes'
        ' [{:id :app.some_module :name "some_module" :ns "app.some_module" :lang :lua'
        f' :file "{source}" :ops [{{:name "M.target"}}]}}] :edges []}}')


def prepare_source_window(project):
    prepare_source_jump(project)


def prepare_inspector(project):
    (project / 'diagram.edn').write_text(
        '{:title "中文 Inspector" :hierarchical true :prefix "demo" :classes'
        ' [{:id :a :name "中文类" :ns "demo.a" :ops [{:name "work" :text "work()"}]}]'
        ' :proposals [{:id :plan :name "中文方案" :layers []}]}')


def prepare_detail_scroll(project):
    ops = ' '.join(f'{{:name "op{n}" :text "op{n}()"}}' for n in range(1, 41))
    (project / 'diagram.edn').write_text(
        '{:title "Scroll Inspector" :hierarchical true :prefix "demo" :classes'
        f' [{{:id :a :name "A" :ns "demo.a" :ops [{ops}]}}]'
        ' :proposals [{:id :plan :name "Plan A" :layers []}]}')


def prepare_fonts(project):
    (project / 'diagram.edn').write_text(
        '{:hierarchical true :prefix "demo" :classes [{:id :a :name "中文类名"'
        ' :ns "demo.a"}] :proposals [{:id :plan :name "中文提案" :layers []}]}')


def prepare_font_parity(project):
    (project / 'diagram.edn').write_text(
        '{:hierarchical true :prefix "demo" :classes [{:id :demo.detail'
        ' :name "中文详情类名" :ns "demo.detail" :level 2 :stereotype :abstract'
        ' :fields ["中文字段"] :ops [{:name "读取" :text "读取中文数据()" :crap 2'
        ' :coverage 0.9 :killed 3 :survived 0}]}]}')


def prepare_dpi_window(project):
    (project / 'diagram.edn').write_text(
        '{:title "DPI 角标" :hierarchical true :prefix "demo" :classes'
        ' [{:id :a :name "中文类" :ns "demo.a" :level 2 :stereotype :interface'
        ' :ops [{:name "work" :text "work()"}]} {:id :b :name "Beta" :ns "demo.b"'
        ' :stereotype :abstract :fields ["count"]}]'
        ' :edges [{:from :a :to :b :kind :dependency}]}')


def prepare_mail(project):
    (project / '.uml-viewer/to-viewer.edn').write_text('{:next-id 1 :queue []}')
    (project / '.metrics').mkdir(parents=True, exist_ok=True)
    for name, title in (('first.edn', 'First diagram'), ('second.edn', 'Second diagram')):
        (project / name).write_text(
            f'{{:title "{title}" :packages [{{:label "P" :classes'
            ' [{:id :target :ns "app.target"}]}]}')


FIXTURES = {
    'metrics': prepare_metrics,
    'navigation': prepare_navigation,
    'search': prepare_search,
    'review_fixes': prepare_review_fixes,
    'source_jump': prepare_source_jump,
    'source_window': prepare_source_window,
    'inspector': prepare_inspector,
    'detail_scroll': prepare_detail_scroll,
    'fonts': prepare_fonts,
    'font_parity': prepare_font_parity,
    'dpi_window': prepare_dpi_window,
    'mail': prepare_mail,
}


def sarasa_font():
    """The provisioned Sarasa font, when the installer has put it in place."""
    path = REPO / '.uml-viewer/fonts/SarasaMonoSC-Regular.ttf'
    return path if path.is_file() else None


def provision_sarasa(project):
    font = sarasa_font()
    if not font:
        raise AssertionError('Sarasa is required by this driver: run get-uml-viewer')
    target = project / '.uml-viewer/fonts'
    target.mkdir(parents=True, exist_ok=True)
    shutil.copy2(font, target / 'SarasaMonoSC-Regular.ttf')
    return font


# Per-driver process environment. The retired wrappers set these; the drivers
# read them, so losing them would silently weaken the assertion.
def env_fonts(project, evidence):
    """Three lookup cases, as the retired wrapper ran them."""
    font = provision_sarasa(project)
    return {'UML_CJK_FONT': '', 'UML_FONT_TEST_EXPECT': 'sarasa',
            'UML_FONT_TEST_EXTRA': '中文类名提案'}


def env_fonts_override(project, evidence):
    font = provision_sarasa(project)
    return {'UML_CJK_FONT': str(font), 'UML_FONT_TEST_EXPECT': 'env',
            'UML_FONT_TEST_EXTRA': '中文类名提案'}


def env_fonts_fallback(project, evidence):
    return {'UML_CJK_FONT': '', 'UML_FONT_TEST_EXPECT': 'system',
            'UML_FONT_TEST_EXTRA': '中文类名提案'}


def env_inspector(project, evidence):
    provision_sarasa(project)
    return {'UML_CJK_FONT': '', 'UML_FONT_TEST_EXPECT': 'sarasa'}


def env_font_parity(project, evidence):
    font = provision_sarasa(project)
    return {'UML_CJK_FONT': str(font), 'UML_PARITY_SCENE': 'chinese'}


def env_font_parity_baseline(project, evidence):
    font = provision_sarasa(project)
    return {'UML_CJK_FONT': str(font), 'UML_PARITY_SCENE': 'baseline'}


def env_source_jump(project, evidence):
    return {'UML68_EVIDENCE': str(evidence), 'UML68_SOURCE_FILE': str(project / 'source.lua')}


def env_review_fixes(project, evidence):
    return {'UML78_EVIDENCE': str(evidence), 'UML78_THROW': '0'}


def env_dpi(project, evidence, phase='first', tag='run', scale='1'):
    provision_sarasa(project)
    return {'UML_CJK_FONT': '', 'UML_VIEWER_TEST_DPI_SCALE': scale,
            'UML_DPI_TEST_PHASE': phase, 'UML_DPI_TEST_TAG': tag}


class DriverSkipped(Exception):
    """The driver's precondition is absent on this machine (not a failure)."""


# Drivers the retired wrappers ran several times, each in its own project. A
# scenario states its own fixture (when it differs from the shared one), the
# environment the driver reads, the viewer arguments and its pass markers.
def scenarios_fonts(project, evidence):
    return [
        {'tag': 'provisioned', 'needs_sarasa': True, 'env': env_fonts},
        {'tag': 'override', 'needs_sarasa': True, 'env': env_fonts_override},
        # The fallback case must run where no Sarasa is provisioned at all.
        {'tag': 'fallback', 'env': env_fonts_fallback},
    ]


def scenarios_font_parity(project, evidence):
    return [
        {'tag': 'baseline', 'needs_sarasa': True, 'env': env_font_parity_baseline,
         'diagram': str(REPO / 'examples/uml-viewer.edn'),
         'script': 'font_parity_baseline'},
        {'tag': 'chinese', 'needs_sarasa': True, 'env': env_font_parity},
    ]


def scenarios_review_fixes(project, evidence):
    return [{'tag': 'clipboard', 'env': env_review_fixes, 'markers': ['clipboard-pass.txt']}]


def scenarios_dpi_window(project, evidence):
    # Scale 1 is the deterministic case; other scales need a real display.
    return [{'tag': 'first-1', 'needs_sarasa': True,
             'env': lambda p, e: env_dpi(p, e, 'first', 'first-1', '1'),
             'markers': ['first-1-pass.edn']}]


# Environments a single-run driver needs. Missing here means the retired
# wrapper's setup would be lost.
DRIVER_ENV = {
    'inspector': env_inspector,
    'detail_scroll': env_inspector,
    'source_jump': env_source_jump,
    'source_window': env_source_jump,
}


SCENARIOS = {
    'fonts': scenarios_fonts,
    'font_parity': scenarios_font_parity,
    'review_fixes': scenarios_review_fixes,
    'dpi_window': scenarios_dpi_window,
}


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
    'metrics': {'seconds': 20, 'evidence': ['metrics-pass.txt', 'scene-1.edn', 'scene-2.edn']},
    'lod_shots': {'seconds': 30, 'evidence': ['lod-blocks.png', 'lod-frames.png',
                                              'lod-full.png', 'renderer.txt']},
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


def edn_script(name):
    """The declarative script of a driver ported to the Rust viewer (#139)."""
    path = REPO / f'scripts/drivers/{name}.edn'
    return path if path.is_file() else None


def lua_driver(name):
    path = REPO / f'scripts/test_love_{name}.lua'
    return path if path.is_file() else None


def rust_executable():
    configured = os.environ.get('UML_VIEWER_RUST_EXECUTABLE')
    if configured:
        return configured
    for profile in ('release', 'debug'):
        binary = REPO / 'target' / profile / 'uml-viewer'
        if binary.is_file():
            return str(binary)
    raise ValueError('the Rust uml-viewer binary is missing: run cargo build --release '
                     '(or set UML_VIEWER_RUST_EXECUTABLE)')


def run_viewer(project, evidence, seconds, driver, diagram, extra_env, tag='run', runtime='love'):
    """Launch the real viewer once and require success plus its pass marker."""
    xdg = Path(tempfile.mkdtemp(prefix='uml-viewer-xdg-'))
    env = dict(os.environ,
               XDG_CONFIG_HOME=str(xdg / 'config'), XDG_DATA_HOME=str(xdg / 'data'),
               XDG_CACHE_HOME=str(xdg / 'cache'), XDG_STATE_HOME=str(xdg / 'state'),
               UML_VIEWER_PROJECT_ROOT=str(project), UML_VIEWER_EVIDENCE=str(evidence),
               UML_VIEWER_TEST_SECONDS=str(seconds),
               UML_VIEWER_TEST_DRIVER=str(driver),
               UML_MAIL_TEST_ROOT=str(project), UML_COMPANION_NO_ATTACH='1')
    env.pop('UML_CIMGUI_DIRECTORY', None)
    if runtime == 'rust':
        argv = [rust_executable(), 'viewer', *diagram]
        # WSLg: without these the viewer falls back to llvmpipe (ADR 0008).
        env.setdefault('GALLIUM_DRIVER', 'd3d12')
        env.setdefault('WGPU_BACKEND', 'gl')
    else:
        argv = [love_executable(), str(REPO / 'love'), 'viewer', *diagram]
    env.update(extra_env)
    try:
        process = subprocess.run(argv, cwd=project, env=env, capture_output=True, text=True,
                                 timeout=seconds + 120)
    finally:
        shutil.rmtree(xdg, ignore_errors=True)
    (evidence / f'{tag}-stdout.txt').write_text(process.stdout or '')
    (evidence / f'{tag}-stderr.txt').write_text(process.stderr or '')
    return process


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('driver', help='driver name, e.g. navigation for test_love_navigation.lua')
    parser.add_argument('--evidence')
    parser.add_argument('--seconds', type=float, default=None)
    parser.add_argument('--project-root', help='use this project instead of an isolated copy')
    parser.add_argument('--diagram', help='initial diagram, relative to the project root')
    parser.add_argument('--pass-marker', action='append', default=[],
                        help='evidence file the driver must create (repeatable)')
    parser.add_argument('--runtime', choices=['love', 'rust'], default=None,
                        help='force a runtime; default picks rust when an EDN script exists')
    arguments = parser.parse_args()

    spec = DRIVERS.get(arguments.driver, {'seconds': 90, 'evidence': []})
    seconds = arguments.seconds or spec['seconds']
    markers = list(arguments.pass_marker) or list(spec['evidence'])
    script = edn_script(arguments.driver)
    runtime = arguments.runtime or ('rust' if script else 'love')
    if runtime == 'rust':
        if not script:
            print(f'no EDN driver: scripts/drivers/{arguments.driver}.edn', file=sys.stderr)
            return 2
        driver = script
    else:
        driver = lua_driver(arguments.driver)
        if not driver:
            print(f'no driver: {REPO}/scripts/test_love_{arguments.driver}.lua', file=sys.stderr)
            return 2
    result_runtime = runtime
    evidence = Path(arguments.evidence or tempfile.mkdtemp(prefix='uml-love-evidence-'))
    evidence.mkdir(parents=True, exist_ok=True)
    temporary_project = arguments.project_root is None
    project = Path(arguments.project_root) if arguments.project_root else \
        Path(tempfile.mkdtemp(prefix='uml love 项目 '))
    result = {'driver': arguments.driver, 'runtime': result_runtime,
              'project': str(project), 'evidence': str(evidence)}
    try:
        if temporary_project:
            (project / '.uml-viewer').mkdir(parents=True)
            if (REPO / '.uml-viewer/cimgui-love-1.90-1').exists():
                shutil.copytree(REPO / '.uml-viewer/cimgui-love-1.90-1',
                                project / '.uml-viewer/cimgui-love-1.90-1')
        fixture = FIXTURES.get(arguments.driver)
        scenarios = SCENARIOS[arguments.driver](project, evidence) \
            if arguments.driver in SCENARIOS else [{'tag': 'run'}]
        for scenario in scenarios:
            tag = scenario['tag']
            # Each scenario starts from the same content. The provisioned font
            # directory in particular is per scenario: a "no Sarasa" case must
            # not inherit the font the previous case placed there.
            shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
            shutil.rmtree(project / '.uml-viewer/fonts', ignore_errors=True)
            if fixture:
                fixture(project)
            if arguments.diagram:
                diagram = [arguments.diagram]
            elif scenario.get('diagram'):
                diagram = [scenario['diagram']]
            else:
                diagram = ['diagram.edn']
            try:
                extra = scenario['env'](project, evidence) if 'env' in scenario \
                    else DRIVER_ENV.get(arguments.driver, lambda p, e: {})(project, evidence)
            except AssertionError as error:
                raise DriverSkipped(str(error))
            # The LOVE driver branches on its environment; an EDN script stays
            # declarative, so a scenario may name its own script instead.
            scenario_driver = driver
            if runtime == 'rust' and scenario.get('script'):
                scenario_driver = edn_script(scenario['script'])
                assert scenario_driver, f"{tag}: no EDN script {scenario['script']}"
            process = run_viewer(project, evidence, seconds, scenario_driver, diagram, extra,
                                 tag=tag, runtime=runtime)
            expected = scenario.get('markers', markers)
            for marker in expected:
                assert (evidence / marker).exists() or (project / marker).exists(), \
                    f'{tag}: driver did not publish {marker}'
            assert process.returncode == 0, \
                f'{tag}: viewer exited {process.returncode}: {(process.stderr or "").strip()[-800:]}'
        result['ok'] = True
    except DriverSkipped as error:
        result['skipped'] = str(error)
    except (AssertionError, subprocess.TimeoutExpired) as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        if (result.get('ok') or result.get('skipped')) and temporary_project:
            shutil.rmtree(project, ignore_errors=True)
        elif not result.get('ok'):
            result['project_retained'] = str(project)
        (evidence / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    if result.get('skipped'):
        print(f'SKIP driver {arguments.driver}: {result["skipped"]}')
        return 0
    if result.get('ok'):
        print(f'PASS driver {arguments.driver}; evidence: {evidence}')
        return 0
    print(f'FAIL driver {arguments.driver}: {result.get("failure")}', file=sys.stderr)
    print(f'evidence: {evidence}', file=sys.stderr)
    return 1


if __name__ == '__main__':
    sys.exit(main())
