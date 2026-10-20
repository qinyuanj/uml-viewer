#!/usr/bin/env python3
"""POSIX viewer acceptance: real LOVE window, real reload, real screenshots.

Runs the viewer through the public entry in an isolated project and asserts
observable results only: the scene files the viewer publishes, the diagram
bytes after a sub-second edit, and the process exit code. Renders a PNG so a
human can confirm WSLg output.

Usage: python3 -I scripts/test_posix_viewer.py [--evidence DIR]
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]


def scene_files(evidence):
    return sorted(evidence.glob('scene-*.edn'))


def wait_for(test, timeout, interval=0.05, message='condition'):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = test()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f'timed out waiting for {message}')


def love_executable():
    love = shutil.which(os.environ.get('UML_LOVE_EXECUTABLE', 'love'))
    if not love:
        raise AssertionError('LOVE 11.5 is required: install love or set UML_LOVE_EXECUTABLE')
    return love


def run_viewer(project, evidence, seconds, driver=None):
    """Launch the viewer directly, like the Windows render harness did.

    The companion lifecycle is deliberately out of scope here (#109): a
    `--backend herdr` launch waits for companion display by design.
    """
    env = dict(os.environ, UML_VIEWER_PROJECT_ROOT=str(project),
               UML_VIEWER_EVIDENCE=str(evidence), UML_VIEWER_TEST_SECONDS=str(seconds))
    env.pop('UML_CIMGUI_DIRECTORY', None)
    if driver:
        env['UML_VIEWER_TEST_DRIVER'] = str(driver)
    return subprocess.Popen([love_executable(), str(REPO / 'love'), 'viewer', 'diagram.edn'],
                            cwd=project, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence')
    arguments = parser.parse_args()
    evidence = Path(arguments.evidence) if arguments.evidence else Path(tempfile.mkdtemp(prefix='uml-viewer-evidence-'))
    evidence.mkdir(parents=True, exist_ok=True)
    project = Path(tempfile.mkdtemp(prefix='uml viewer 项目 '))
    result = {'project': str(project), 'evidence': str(evidence), 'checks': []}
    try:
        shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
        (project / '.uml-viewer').mkdir()
        shutil.copytree(REPO / '.uml-viewer/cimgui-love-1.90-1', project / '.uml-viewer/cimgui-love-1.90-1')

        process = run_viewer(project, evidence, 45)
        first = wait_for(lambda: scene_files(evidence), 40, message='first scene')
        result['checks'].append(('first scene published', str(first[0].name)))

        # A same-second edit must still be observed: the POSIX stamp carries
        # nanoseconds, so no sleep is needed between the two writes.
        path = project / 'diagram.edn'
        original = path.read_text()
        path.write_text(original.replace('UML viewer', 'Reloaded UML viewer'))
        second = wait_for(lambda: [p for p in scene_files(evidence) if p.name != first[0].name], 40,
                          message='reload scene')
        result['checks'].append(('reload scene published', str(second[0].name)))
        reloaded = second[0].read_text()
        assert 'Reloaded UML viewer' in reloaded, 'reloaded scene lost the edited title'

        # The window is actually rendering: the viewer captures its own frame,
        # which is the same pixels WSLg presents to the user.
        shot = wait_for(lambda: (evidence / 'render.png').exists() and evidence / 'render.png', 20,
                        message='rendered frame')
        result['checks'].append(('rendered frame', f'{shot} {shot.stat().st_size} bytes '
                                  f'sha256={hashlib.sha256(shot.read_bytes()).hexdigest()[:16]}'))

        # The viewer leaves on its own schedule (UML_VIEWER_TEST_SECONDS), the
        # same love.quit path a user's window close takes. SIGKILL is never used;
        # a viewer that misses its deadline is a failure, not something to wait
        # out.
        try:
            code = process.wait(timeout=90)
        except subprocess.TimeoutExpired:
            process.terminate()
            try:
                code = process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                raise AssertionError('viewer ignored its own test duration and SIGTERM')
            raise AssertionError(f'viewer missed its test duration (exit {code})')
        out, err = process.communicate()
        (evidence / 'viewer-stdout.txt').write_text(out or '')
        (evidence / 'viewer-stderr.txt').write_text(err or '')
        assert code == 0, f'viewer exited {code}: {err}'
        result['checks'].append(('viewer exit', str(code)))
        result['ok'] = True
    except AssertionError as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        (evidence / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        shutil.rmtree(project, ignore_errors=True)
    for name, detail in result['checks']:
        print(f'PASS {name}: {detail}')
    if not result.get('ok'):
        print('FAIL ' + result.get('failure', ''), file=sys.stderr)
        print('evidence: ' + str(evidence), file=sys.stderr)
        return 1
    print('evidence: ' + str(evidence))
    return 0


if __name__ == '__main__':
    sys.exit(main())
