#!/usr/bin/env python3
"""POSIX viewer acceptance: real Rust window, real reload, real screenshots.

Runs the viewer through the public entry in an isolated project and asserts
observable results only: the scene files the viewer publishes, the diagram
bytes after a sub-second edit, and the process exit code. Renders a PNG so a
human can confirm WSLg output.

Usage: python3 -I scripts/test_posix_viewer.py [--evidence DIR] [--only reload|idle-sigterm]
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


def rust_executable():
    configured = os.environ.get('UML_VIEWER_RUST_EXECUTABLE')
    if configured:
        return configured
    for profile in ('release', 'debug'):
        binary = REPO / 'target' / profile / 'uml-viewer'
        if binary.is_file():
            return str(binary)
    raise AssertionError('the Rust uml-viewer binary is missing: run cargo build --release '
                         '(or set UML_VIEWER_RUST_EXECUTABLE)')


def run_viewer(project, evidence, seconds, driver=None):
    """Launch the viewer directly, like the Windows render harness did.

    The companion lifecycle is deliberately out of scope here (#109): a
    `--backend herdr` launch waits for companion display by design. Without
    `seconds` the viewer has no test duration, so nothing keeps it repainting.
    """
    env = dict(os.environ, UML_VIEWER_PROJECT_ROOT=str(project),
               UML_VIEWER_EVIDENCE=str(evidence))
    if seconds is not None:
        env['UML_VIEWER_TEST_SECONDS'] = str(seconds)
    env.setdefault('GALLIUM_DRIVER', 'd3d12')
    env.setdefault('WGPU_BACKEND', 'gl')
    if driver:
        env['UML_VIEWER_TEST_DRIVER'] = str(driver)
    return subprocess.Popen([rust_executable(), 'viewer', 'diagram.edn'],
                            cwd=project, env=env, text=True,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)


def main_thread_ticks(pid):
    """utime + stime of the main (UI) thread, in clock ticks. Frames run on
    this thread, so its CPU is the repaint activity; helper threads that only
    stat files are not."""
    fields = Path(f'/proc/{pid}/task/{pid}/stat').read_text().rsplit(')', 1)[1].split()
    return int(fields[11]) + int(fields[12])


def wait_idle(process, timeout=40, window=2.0, ticks=1, windows=2):
    """Idle means no repaint activity: the UI thread burns (almost) no CPU over
    several consecutive windows. Startup frames and a repaint cadence both
    fail this check, so the signal is sent only to a truly idle viewer."""
    deadline = time.monotonic() + timeout
    quiet = 0
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError(f'viewer exited before it became idle ({process.returncode})')
        before = main_thread_ticks(process.pid)
        time.sleep(window)
        spent = main_thread_ticks(process.pid) - before
        quiet = quiet + 1 if spent <= ticks else 0
        if quiet >= windows:
            return spent
    raise AssertionError('viewer never became idle')


def check_idle_sigterm(project, evidence, result):
    """SIGTERM to an idle viewer is a normal close within 2 s (exit 0).

    An idle viewer requests no frames, so a flag the frame loop reads would
    only be seen at the next unrelated repaint. On failure the viewer is woken
    by touching its diagram (it then honours the pending signal); SIGKILL is
    never used.
    """
    idle = evidence / 'idle-sigterm'
    idle.mkdir(parents=True, exist_ok=True)
    process = run_viewer(project, idle, None)
    try:
        wait_for(lambda: (idle / 'render.png').exists(), 40, message='idle viewer first frame')
        spent = wait_idle(process)
        result['checks'].append(('viewer idle before SIGTERM', f'{spent} main-thread ticks in the last 2 s'))
        process.terminate()
        sent = time.monotonic()
        try:
            code = process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            raise AssertionError('idle viewer ignored SIGTERM for 2 s')
        elapsed = time.monotonic() - sent
        assert code == 0, f'idle viewer exited {code} on SIGTERM'
        result['checks'].append(('idle SIGTERM exit', f'{code} after {elapsed:.2f} s'))
    finally:
        if process.poll() is None:
            path = project / 'diagram.edn'
            path.write_text(path.read_text())
            try:
                process.wait(timeout=20)
            except subprocess.TimeoutExpired:
                process.terminate()
                process.wait(timeout=20)
        out, err = process.communicate()
        (idle / 'viewer-stdout.txt').write_text(out or '')
        (idle / 'viewer-stderr.txt').write_text(err or '')


def check_reload(project, evidence, result):
    """A sub-second diagram edit reloads, a frame renders, and the test
    duration closes the viewer normally (exit 0)."""
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
    # same quit path a user's window close takes. SIGKILL is never used;
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


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence')
    parser.add_argument('--only', choices=['reload', 'idle-sigterm'],
                        help='run a single check instead of all of them')
    arguments = parser.parse_args()
    evidence = Path(arguments.evidence) if arguments.evidence else Path(tempfile.mkdtemp(prefix='uml-viewer-evidence-'))
    evidence.mkdir(parents=True, exist_ok=True)
    project = Path(tempfile.mkdtemp(prefix='uml viewer 项目 '))
    result = {'project': str(project), 'evidence': str(evidence), 'checks': []}
    try:
        shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
        (project / '.uml-viewer').mkdir()
        if arguments.only != 'idle-sigterm':
            check_reload(project, evidence, result)
        if arguments.only != 'reload':
            check_idle_sigterm(project, evidence, result)
        result['ok'] = True
    except AssertionError as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        (evidence / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
        shutil.rmtree(project, ignore_errors=True)
    return report(result, evidence)


def report(result, evidence):
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
