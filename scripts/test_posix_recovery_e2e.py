#!/usr/bin/env python3
"""真实 durable recovery 端到端：公共 ./uml → 异常退出 → 有限恢复 → 关闭取消。

只断言用户可观察的结果：companion 子进程的实际启动时刻、恢复日志的阶段与
次数、controller 进程是否存活、邮件队列出队、ownership 文件与 Herdr 会话
清单，以及一个无关 Herdr 会话和无关进程始终存活。companion 是一个按指令以
指定退出码退出的占位进程；真实 Claude 会话的证据另行记录。

一个旁观项目全程运行；三个隔离项目依次运行：
- cancel：两次异常退出各在 1/2 秒预算后恢复，恢复后邮件继续出队；第三次
  异常退出进入 4 秒退避，退避期间正常关闭 viewer，恢复被取消且不再复活。
- exhausted：连续四次异常退出，三次恢复后停止，不再启动第五个子进程。
- normal：子进程以 0 退出，恢复停止，不重新启动。
- unknown：runner 被挂断（SIGHUP）而未记录退出，随后子进程消失；退出原因
  无法核实，恢复停止，不重新启动。
最后核对旁观项目的 viewer、ownership、Herdr 会话与 companion 子进程原样存活，
再正常关闭它。
"""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time

REPO = Path(__file__).resolve().parents[1]
BACKOFF = (1.0, 2.0, 4.0)
# The controller polls every 100 ms; its scheduled delay from the runner's
# recorded exit to the retry deadline must be the attempt's budget within this.
SCHEDULE_TOLERANCE = 0.5
# A relaunch resets the pane terminal and starts a viewer runner before the
# child exists; this bounds that overhead without fixing it as a contract.
RELAUNCH_SLACK = 6.0

PLACEHOLDER = '''#!/bin/sh
here=$(dirname "$0")
echo "$(date +%s.%N) $$" >> "$here/starts.log"
while :; do
  if [ -f "$here/exit-code" ]; then
    code=$(cat "$here/exit-code"); rm -f "$here/exit-code"; exit "$code"
  fi
  sleep 0.05
done
'''


def wait_for(test, timeout, interval=0.1, message='condition'):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        value = test()
        if value:
            return value
        time.sleep(interval)
    raise AssertionError(f'timed out waiting for {message}')


def alive(pid, start=None):
    try:
        stat = Path(f'/proc/{pid}/stat').read_text()
    except OSError:
        return False
    fields = stat.rsplit(')', 1)[1].split()
    # An exited child its parent has not reaped yet is gone, not running.
    if fields[0] in ('Z', 'X'):
        return False
    return start is None or fields[19] == start


def journal(project):
    path = project / '.uml-viewer/recovery.edn'
    if not path.exists():
        return {}
    text = path.read_text()
    found = {}
    match = re.search(r':phase :([a-z-]+)', text)
    if match:
        found['phase'] = match.group(1)
    for key in ('attempts', 'generation'):
        match = re.search(r':' + key + r' (\d+)', text)
        if match:
            found[key] = int(match.group(1))
    match = re.search(r':controller \{([^}]*)\}', text)
    if match:
        pid = re.search(r':pid (\d+)', match.group(1))
        start = re.search(r':start "(\d+)"', match.group(1))
        found['controller'] = (int(pid.group(1)), start.group(1))
    match = re.search(r':reason "([^"]*)"', text)
    if match:
        found['reason'] = match.group(1)
    match = re.search(r':deadline (\d+)', text)
    if match:
        found['deadline'] = int(match.group(1))
    match = re.search(r':runner-state "([^"]*)"', text)
    if match:
        found['runner_state'] = match.group(1)
    return found


def runner_exit_at(path):
    """The runner's own monotonic record of a verified child exit, if any."""
    try:
        text = Path(path).read_text()
    except OSError:
        return None
    if ':phase :exited' not in text:
        return None
    match = re.search(r':at (\d+)', text)
    return int(match.group(1)) if match else None


def parent(pid):
    status = Path(f'/proc/{pid}/status').read_text()
    return int(re.search(r'^PPid:\s+(\d+)', status, re.M).group(1))


def starts(project):
    path = project / 'starts.log'
    if not path.exists():
        return []
    return [(float(line.split()[0]), int(line.split()[1]))
            for line in path.read_text().splitlines() if line.strip()]


def ownership(project):
    path = project / '.uml-viewer/companion.edn'
    if not path.exists():
        return None
    text = path.read_text().strip()
    return None if text in ('', '{}') else text


def sessions(env):
    result = subprocess.run(['herdr', 'session', 'list', '--json'], capture_output=True, text=True, env=env)
    if result.returncode != 0:
        return []
    return [entry['name'] for entry in json.loads(result.stdout).get('sessions', []) if entry.get('running')]


def request_exit(project, code):
    """Ask the current companion child to exit with code; return the request time."""
    temporary = project / 'exit-code.tmp'
    temporary.write_text(str(code))
    requested = time.time()
    temporary.rename(project / 'exit-code')
    return requested


class Viewer:
    def __init__(self, name, runtime, evidence, checks):
        self.name = name
        self.checks = checks
        self.evidence = evidence / name
        self.evidence.mkdir(parents=True, exist_ok=True)
        self.project = Path(tempfile.mkdtemp(prefix=f'uml recovery {name} 项目 '))
        self.env = dict(os.environ,
                        XDG_CONFIG_HOME=str(runtime / 'config'), XDG_DATA_HOME=str(runtime / 'data'),
                        XDG_CACHE_HOME=str(runtime / 'cache'), XDG_STATE_HOME=str(runtime / 'state'),
                        UML_VIEWER_TOOL_ROOT=str(REPO), UML_VIEWER_EVIDENCE=str(self.evidence),
                        UML_VIEWER_TEST_SECONDS='600',
                        CLAUDE_BIN=str(self.project / 'companion-placeholder'),
                        UML_COMPANION_NO_ATTACH='1')
        for variable in ('UML_VIEWER_PROJECT_ROOT',):
            self.env.pop(variable, None)
        self.launcher = None
        self.session = None

    def check(self, label, detail):
        self.checks.append((f'{self.name}: {label}', detail))

    def launch(self):
        project = self.project
        (project / '.uml-viewer').mkdir()
        shutil.copy2(REPO / 'examples/uml-viewer.edn', project / 'diagram.edn')
        (project / 'companion-placeholder').write_text(PLACEHOLDER)
        (project / 'companion-placeholder').chmod(0o755)
        shutil.copy2(REPO / 'scripts/uml', project / 'uml')
        (project / 'uml').chmod(0o755)
        launched = subprocess.run([str(project / 'uml')], cwd=project, env=self.env,
                                  capture_output=True, text=True)
        assert launched.returncode == 0, f'{self.name}: viewer launch failed: {launched.stderr}'
        self.launcher = int(launched.stdout.split('launcher pid ')[1].split(')')[0])
        record = wait_for(lambda: ownership(project), 90, message=f'{self.name} companion ownership')
        self.session = record.split(':session "')[1].split('"')[0]
        wait_for(lambda: any(':phase :ready' in path.read_text()
                             for path in (project / '.uml-viewer').glob('companion-worker-*.event-*.edn')),
                 120, message=f'{self.name} companion ready')
        wait_for(lambda: len(starts(project)) == 1, 30, message=f'{self.name} first child')
        wait_for(lambda: journal(project).get('controller'), 30, message=f'{self.name} controller')
        self.check('companion ready', self.session)

    def crash_and_recover(self, attempt, code=7):
        """One abnormal exit; the next child must start after the attempt's budget."""
        before = len(starts(self.project))
        requested = request_exit(self.project, code)
        # The retry deadline the controller journals, against the exit the
        # runner recorded on the same monotonic clock.
        backoff = wait_for(lambda: (lambda value: value if value.get('phase') == 'backoff'
                                    and runner_exit_at(value.get('runner_state', '')) else None)(
                               journal(self.project)),
                           BACKOFF[attempt - 1], interval=0.01, message=f'{self.name} backoff {attempt}')
        scheduled = (backoff['deadline'] - runner_exit_at(backoff['runner_state'])) / 1000
        assert BACKOFF[attempt - 1] <= scheduled <= BACKOFF[attempt - 1] + SCHEDULE_TOLERANCE, \
            f'{self.name}: retry {attempt} scheduled {scheduled:.2f}s after exit; budget {BACKOFF[attempt - 1]}s'
        observed = wait_for(lambda: len(starts(self.project)) > before and starts(self.project)[-1],
                            BACKOFF[attempt - 1] + RELAUNCH_SLACK + 10, interval=0.05,
                            message=f'{self.name} recovery {attempt}')
        gap = observed[0] - requested
        assert BACKOFF[attempt - 1] <= gap <= BACKOFF[attempt - 1] + RELAUNCH_SLACK, \
            f'{self.name}: recovery {attempt} started {gap:.2f}s after exit; budget {BACKOFF[attempt - 1]}s'
        state = wait_for(lambda: (lambda value: value if value.get('phase') == 'running'
                                  and value.get('attempts') == attempt else None)(journal(self.project)),
                         10, message=f'{self.name} journal after recovery {attempt}')
        self.check(f'recovery {attempt}', f'scheduled {scheduled:.2f}s after exit (budget '
                                          f'{BACKOFF[attempt - 1]:.0f}s), child started {gap:.2f}s after, '
                                          f'attempts {state["attempts"]}, generation {state["generation"]}')

    def assert_no_new_child(self, count, seconds, label):
        time.sleep(seconds)
        assert len(starts(self.project)) == count, \
            f'{self.name}: {label}: a child started after recovery stopped ({starts(self.project)})'

    def close(self):
        """A normal window close: the window manager's quit request to the viewer."""
        os.kill(self.launcher, signal.SIGTERM)
        wait_for(lambda: not alive(self.launcher), 60, message=f'{self.name} viewer exit')
        wait_for(lambda: ownership(self.project) is None, 90, message=f'{self.name} companion cleanup')
        wait_for(lambda: self.session not in sessions(self.env), 30, message=f'{self.name} herdr session removal')
        self.check('closed', 'viewer exited, ownership {} and Herdr session removed')

    def keep_logs(self):
        for name in ('viewer.log', 'recovery.edn', 'recovery.log', 'recovery.log.err'):
            source = self.project / '.uml-viewer' / name
            if source.exists():
                shutil.copy2(source, self.evidence / name)
        if (self.project / 'starts.log').exists():
            shutil.copy2(self.project / 'starts.log', self.evidence / 'starts.log')


def scenario_cancel(viewer):
    viewer.launch()
    project = viewer.project
    viewer.crash_and_recover(1)
    # Mail is still consumed oldest first while the companion is recovered.
    (project / '.uml-viewer/to-viewer.edn').write_text(
        '{:next-id 3 :queue [{:id 1 :op :context :context :real} {:id 2 :op :display :path "diagram.edn"}]}')
    wait_for(lambda: ':queue []' in (project / '.uml-viewer/to-viewer.edn').read_text(), 60,
             message='FIFO drain after recovery')
    # Mesa writes libEGL/MESA-LOADER warnings to the same log while wgpu probes
    # the GPU; they are not viewer verdicts about mail.
    noise = ('libEGL warning', 'MESA-LOADER')
    rejected = [line for line in (project / '.uml-viewer/viewer.log').read_text().splitlines()
                if ('rejected' in line or 'failed' in line)
                and not line.lstrip().startswith(noise)]
    assert not rejected, f'mail after recovery was rejected: {rejected}'
    viewer.check('FIFO after recovery', 'queue drained oldest first, no rejection logged')
    viewer.crash_and_recover(2)
    controller = journal(project)['controller']
    count = len(starts(project))
    exited = request_exit(project, 9)
    wait_for(lambda: journal(project).get('phase') == 'backoff', 10, interval=0.02,
             message='third backoff')
    closing = time.time() - exited
    assert closing < BACKOFF[2], f'close requested {closing:.2f}s after exit, outside the 4s backoff'
    viewer.check('third exit', f'journal :backoff; close requested {closing:.2f}s after exit (4s budget)')
    viewer.close()
    state = journal(project)
    # Ownership is cleared after cleanup; the journal remains as the record.
    assert state.get('phase') == 'cancelled', f'recovery not cancelled: {state}'
    assert not alive(*controller), f'controller {controller} survived close'
    viewer.check('close during backoff', f'journal :cancelled, controller {controller[0]} gone')
    viewer.assert_no_new_child(count, BACKOFF[2] + RELAUNCH_SLACK, 'close during backoff')
    viewer.check('no revival', f'{count} children after the 4s budget elapsed')


def scenario_exhausted(viewer):
    viewer.launch()
    project = viewer.project
    for attempt in (1, 2, 3):
        viewer.crash_and_recover(attempt)
    controller = journal(project)['controller']
    request_exit(project, 11)
    state = wait_for(lambda: (lambda value: value if value.get('phase') == 'failed' else None)(journal(project)),
                     20, message='exhaustion')
    assert 'exhausted' in state.get('reason', ''), f'unexpected stop: {state}'
    wait_for(lambda: not alive(*controller), 10, message='controller exit after exhaustion')
    viewer.check('fourth exit', f'journal :failed "{state["reason"]}", controller gone')
    viewer.assert_no_new_child(4, BACKOFF[2] + RELAUNCH_SLACK, 'exhausted')
    viewer.check('no fifth child', '4 children in total')
    viewer.close()


def scenario_normal(viewer):
    viewer.launch()
    project = viewer.project
    controller = journal(project)['controller']
    request_exit(project, 0)
    state = wait_for(lambda: (lambda value: value if value.get('phase') == 'normal' else None)(journal(project)),
                     20, message='normal exit')
    wait_for(lambda: not alive(*controller), 10, message='controller exit after normal exit')
    viewer.check('exit 0', f'journal :normal "{state.get("reason")}", controller gone')
    viewer.assert_no_new_child(1, BACKOFF[0] + RELAUNCH_SLACK, 'normal exit')
    viewer.check('no relaunch', '1 child in total')
    viewer.close()


def scenario_unknown(viewer):
    viewer.launch()
    project = viewer.project
    controller = journal(project)['controller']
    child = starts(project)[0][1]
    runner = parent(child)
    # A hangup is the terminal going away: the runner dies before it can
    # record an exit, and the child then disappears on its own.
    os.kill(runner, signal.SIGHUP)
    wait_for(lambda: not alive(runner), 10, message='runner hangup')
    os.kill(child, signal.SIGTERM)
    state = wait_for(lambda: (lambda value: value if value.get('phase') == 'failed' else None)(journal(project)),
                     20, message='unverified exit')
    assert 'without a matching exit' in state.get('reason', ''), f'unexpected stop: {state}'
    wait_for(lambda: not alive(*controller), 10, message='controller exit after unverified exit')
    viewer.check('unverified exit', f'journal :failed "{state["reason"]}", controller gone')
    viewer.assert_no_new_child(1, BACKOFF[0] + RELAUNCH_SLACK, 'unverified exit')
    viewer.check('no relaunch', '1 child in total')
    viewer.close()


SCENARIOS = {'cancel': scenario_cancel, 'exhausted': scenario_exhausted, 'normal': scenario_normal,
             'unknown': scenario_unknown}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence')
    parser.add_argument('scenarios', nargs='*', default=list(SCENARIOS))
    arguments = parser.parse_args()
    evidence = Path(arguments.evidence or tempfile.mkdtemp(prefix='uml-recovery-evidence-')).resolve()
    evidence.mkdir(parents=True, exist_ok=True)
    runtime = Path(tempfile.mkdtemp(prefix='uml-rec-xdg-'))
    checks = []
    result = {'evidence': str(evidence), 'runtime': str(runtime), 'projects': {}}
    viewers = []
    bystander_env = dict(os.environ, XDG_CONFIG_HOME=str(runtime / 'config'))
    # Unrelated resources that must survive every scenario: a Herdr session in
    # the same runtime and an ordinary process.
    bystander = subprocess.Popen(['herdr', '--session', 'bystander', 'server'], env=bystander_env,
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, cwd=runtime)
    sleeper = subprocess.Popen(['sleep', '3600'])
    neighbour = Viewer('neighbour', runtime, evidence, checks)
    viewers.append(neighbour)
    result['projects']['neighbour'] = str(neighbour.project)
    try:
        wait_for(lambda: 'bystander' in sessions(bystander_env), 10, message='bystander Herdr session')
        neighbour.launch()
        neighbour_record = ownership(neighbour.project)
        neighbour_child = starts(neighbour.project)
        for name in arguments.scenarios:
            viewer = Viewer(name, runtime, evidence, checks)
            viewers.append(viewer)
            result['projects'][name] = str(viewer.project)
            try:
                SCENARIOS[name](viewer)
            finally:
                viewer.keep_logs()
            assert 'bystander' in sessions(bystander_env), f'{name}: unrelated Herdr session was stopped'
            assert bystander.poll() is None and sleeper.poll() is None, f'{name}: unrelated process stopped'
            assert alive(neighbour.launcher) and ownership(neighbour.project) == neighbour_record \
                and neighbour.session in sessions(neighbour.env), f'{name}: another project was disturbed'
            checks.append((f'{name}: bystanders', 'unrelated Herdr session, process and project alive'))
        assert starts(neighbour.project) == neighbour_child and alive(neighbour_child[0][1]), \
            'the other project\'s companion was restarted or stopped'
        neighbour.check('undisturbed', 'same ownership, Herdr session and companion child throughout')
        neighbour.close()
        result['ok'] = True
    except AssertionError as error:
        result['ok'] = False
        result['failure'] = str(error)
    finally:
        neighbour.keep_logs()
        sleeper.terminate()
        subprocess.run(['herdr', '--session', 'bystander', 'server', 'stop'], env=bystander_env,
                       capture_output=True, timeout=30)
        try:
            bystander.wait(timeout=30)
        except subprocess.TimeoutExpired:
            bystander.terminate()
        if result.get('ok'):
            for viewer in viewers:
                shutil.rmtree(viewer.project, ignore_errors=True)
            shutil.rmtree(runtime, ignore_errors=True)
        else:
            # A failed run still closes its viewers the normal way, so live
            # companions release what they verifiably own; whatever a failed
            # startup retained stays with the runtime for diagnosis.
            for viewer in viewers:
                if viewer.launcher and alive(viewer.launcher):
                    os.kill(viewer.launcher, signal.SIGTERM)
            result['runtime_retained'] = str(runtime)
        result['checks'] = [f'{name}: {detail}' for name, detail in checks]
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
