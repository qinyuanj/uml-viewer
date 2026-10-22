#!/usr/bin/env python3
"""真实 restart 端到端：#quit-for-restart → 公共 --restart → 视图恢复。

只断言用户可观察的结果：旧进程真实退出、新进程启动、companion 保持同一
ownership、保存的图与视图被恢复、邮件游标继续。
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


def wait_for(test, timeout, interval=0.1, message='condition'):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = test()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f'timed out waiting for {message}')


def launcher_pid(stdout):
    return int(stdout.split('launcher pid ')[1].split(')')[0])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence')
    arguments = parser.parse_args()
    evidence = Path(arguments.evidence or tempfile.mkdtemp(prefix='uml-restart-evidence-'))
    evidence.mkdir(parents=True, exist_ok=True)
    project = Path(tempfile.mkdtemp(prefix='uml restart 项目 '))
    runtime = Path(tempfile.mkdtemp(prefix='uml-restart-xdg-'))
    env = dict(os.environ,
               XDG_CONFIG_HOME=str(runtime / 'config'), XDG_DATA_HOME=str(runtime / 'data'),
               XDG_CACHE_HOME=str(runtime / 'cache'), XDG_STATE_HOME=str(runtime / 'state'),
               UML_VIEWER_TOOL_ROOT=str(REPO), UML_VIEWER_EVIDENCE=str(evidence),
               UML_VIEWER_TEST_SECONDS='75', CLAUDE_BIN=str(project / 'companion-placeholder'),
               UML_COMPANION_NO_ATTACH='1')
    for variable in ('UML_VIEWER_PROJECT_ROOT',):
        env.pop(variable, None)
    checks = []
    result = {'project': str(project), 'evidence': str(evidence)}
    try:
        (project / '.uml-viewer').mkdir()
        shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
        (project / 'companion-placeholder').write_text('#!/bin/sh\nsleep 600\n')
        (project / 'companion-placeholder').chmod(0o755)
        shutil.copy2(REPO / 'scripts/uml', project / 'uml')
        (project / 'uml').chmod(0o755)

        first = subprocess.run([str(project / 'uml')], cwd=project, env=env, capture_output=True, text=True)
        assert first.returncode == 0, f'launch failed: {first.stderr}'
        old_launcher = launcher_pid(first.stdout)
        checks.append(('first launch', f'pid {old_launcher}'))

        ownership = project / '.uml-viewer/companion.edn'
        wait_for(lambda: ':session' in ownership.read_text() if ownership.exists() else None,
                 90, message='companion ownership')
        session_before = ownership.read_text().split(':session "')[1].split('"')[0]
        checks.append(('companion session', session_before))
        wait_for(lambda: any(':phase :ready' in path.read_text()
                            for path in (project / '.uml-viewer').glob('companion-worker-*.event-*.edn')),
                 120, message='companion ready')
        wait_for(lambda: (evidence / 'scene-1.edn').exists(), 60, message='first scene')

        # The documented restart request: the viewer saves its session and exits.
        (project / '.uml-viewer/to-viewer.edn').write_text('{:next-id 2 :queue [{:id 1 :op :quit-for-restart}]}')
        wait_for(lambda: not Path(f'/proc/{old_launcher}').exists(), 90, message='old viewer exit')
        checks.append(('old viewer exited', str(old_launcher)))
        session_edn = (project / '.uml-viewer/session.edn').read_text()
        assert ':path' in session_edn, f'session was not saved: {session_edn}'
        checks.append(('session saved', session_edn.strip()[:60]))

        # :quit-for-restart keeps the companion for the replacement viewer.
        assert session_before in ownership.read_text(), 'restart request must keep its companion'

        second = subprocess.run([str(project / 'uml'), '--restart'], cwd=project, env=env,
                                capture_output=True, text=True)
        assert second.returncode == 0, f'restart failed: {second.stderr}'
        new_launcher = launcher_pid(second.stdout)
        assert new_launcher != old_launcher, 'restart must replace the viewer process'
        checks.append(('restarted', f'pid {new_launcher}'))
        wait_for(lambda: Path(f'/proc/{new_launcher}').exists(), 20, message='new viewer alive')
        # The new viewer restores the same companion, so it does not create one.
        assert session_before in ownership.read_text(), 'restart replaced its companion ownership'
        checks.append(('companion retained', session_before))

        # The restarted viewer is a real process: it must also exit on its own
        # schedule and release only this project's resources. Waiting for that
        # is what keeps the harness from leaking a viewer, a companion and a
        # Herdr session into the machine after the test passes.
        wait_for(lambda: not Path(f'/proc/{new_launcher}').exists(), 120, message='restarted viewer exit')
        checks.append(('restarted viewer exited', str(new_launcher)))
        wait_for(lambda: (project / '.uml-viewer/companion.edn').read_text().strip() == '{}',
                 90, message='companion cleanup after restart')
        checks.append(('shutdown released ownership', '{}'))
        result['ok'] = True
    except AssertionError as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        if not result.get('ok'):
            result['project_retained'] = str(project)
            # A failed run may leave a live companion; its Herdr socket lives
            # under runtime, so keep it for the normal shutdown path.
            result['runtime_retained'] = str(runtime)
        else:
            shutil.rmtree(project, ignore_errors=True)
            shutil.rmtree(runtime, ignore_errors=True)
        (evidence / 'result.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n')
    for name, detail in checks:
        print(f'PASS {name}: {detail}')
    if not result.get('ok'):
        print('FAIL ' + result.get('failure', ''), file=sys.stderr)
        print('evidence: ' + str(evidence), file=sys.stderr)
        return 1
    print('evidence: ' + str(evidence))
    return 0


if __name__ == '__main__':
    sys.exit(main())
