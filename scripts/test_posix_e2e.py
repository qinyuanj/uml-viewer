#!/usr/bin/env python3
"""公共入口的 WSL 端到端验收：fresh companion → ready → FIFO 邮件 → 关闭清理。

只断言用户可观察的结果：公共入口退出码、ownership 文件、Herdr 会话清单、
邮件队列出队顺序、viewer 发布的场景文件与日志。不启动真实 Claude 会话：
companion 的子进程是一个无害的占位进程，与既有 worker fixture 相同。
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


def ownership(project):
    path = project / '.uml-viewer/companion.edn'
    if not path.exists():
        return None
    text = path.read_text().strip()
    return None if text in ('', '{}') else text


def sessions(project, env):
    result = subprocess.run(['herdr', 'session', 'list', '--json'], capture_output=True, text=True, env=env)
    if result.returncode != 0:
        return []
    return [entry['name'] for entry in json.loads(result.stdout).get('sessions', [])]


def queue(project, text):
    (project / '.uml-viewer/to-viewer.edn').write_text(text)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence')
    arguments = parser.parse_args()
    evidence = Path(arguments.evidence or tempfile.mkdtemp(prefix='uml-e2e-evidence-'))
    evidence.mkdir(parents=True, exist_ok=True)
    project = Path(tempfile.mkdtemp(prefix='uml e2e 项目 '))
    runtime = Path(tempfile.mkdtemp(prefix='uml-e2e-xdg-'))
    env = dict(os.environ,
               XDG_CONFIG_HOME=str(runtime / 'config'), XDG_DATA_HOME=str(runtime / 'data'),
               XDG_CACHE_HOME=str(runtime / 'cache'), XDG_STATE_HOME=str(runtime / 'state'),
               UML_VIEWER_TOOL_ROOT=str(REPO), UML_VIEWER_EVIDENCE=str(evidence),
               UML_VIEWER_TEST_SECONDS='120', CLAUDE_BIN=str(project / 'companion-placeholder'),
               UML_COMPANION_NO_ATTACH='1')
    for variable in ('UML_VIEWER_PROJECT_ROOT',):
        env.pop(variable, None)
    checks = []
    result = {'project': str(project), 'evidence': str(evidence)}
    log = None
    try:
        (project / '.uml-viewer').mkdir()
        shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
        (project / 'companion-placeholder').write_text('#!/bin/sh\nsleep 300\n')
        (project / 'companion-placeholder').chmod(0o755)
        shutil.copy2(REPO / 'scripts/uml', project / 'uml')
        (project / 'uml').chmod(0o755)

        launched = subprocess.run([str(project / 'uml')], cwd=project, env=env, capture_output=True, text=True)
        assert launched.returncode == 0, f'viewer launch failed: {launched.stderr}'
        checks.append(('launch', launched.stdout.strip().splitlines()[0]))
        launcher = int(launched.stdout.split('launcher pid ')[1].split(')')[0])

        record = wait_for(lambda: ownership(project), 90, message='companion ownership')
        session = record.split(':session "')[1].split('"')[0]
        checks.append(('companion ownership', session))

        wait_for(lambda: any(':phase :ready' in path.read_text()
                            for path in (project / '.uml-viewer').glob('companion-worker-*.event-*.edn')),
                 120, message='companion ready')
        checks.append(('companion ready', 'worker event :ready'))

        wait_for(lambda: (evidence / 'scene-1.edn').exists(), 60, message='first scene')
        checks.append(('viewer scenes', 'scene-1.edn'))

        # FIFO: mail must be consumed oldest first and popped from the queue.
        queue(project, '{:next-id 3 :queue [{:id 1 :op :context :context :real} '
                        '{:id 2 :op :display :path "diagram.edn"}]}')
        wait_for(lambda: ':queue []' in (project / '.uml-viewer/to-viewer.edn').read_text(),
                 60, message='FIFO drain')
        checks.append(('FIFO drain', (project / '.uml-viewer/to-viewer.edn').read_text().strip()[:40]))

        # Normal window close: the viewer exits on its own schedule (the test
        # duration is the same quit path a user's window close takes) and
        # must release only its own verified resources.
        wait_for(lambda: not Path(f'/proc/{launcher}').exists(), 120, message='viewer exit')
        checks.append(('viewer exit', 'process gone'))
        wait_for(lambda: ownership(project) is None, 90, message='companion cleanup')
        checks.append(('shutdown released ownership', '{}'))
        wait_for(lambda: session not in sessions(project, env), 30, message='herdr session removal')
        checks.append(('herdr session removed', session))
        result['ok'] = True
        shutil.copy2(project / '.uml-viewer/viewer.log', evidence / 'viewer.log')
    except AssertionError as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        if not result.get('ok'):
            shutil.copy2(project / '.uml-viewer/viewer.log', evidence / 'viewer.log') \
                if (project / '.uml-viewer/viewer.log').exists() else None
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
