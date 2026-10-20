#!/usr/bin/env python3
"""One guardrail command: the checks that are safe to run at any time.

This is the only guardrail command; the earlier `check.ps1` was retired with
Windows native support (ADR 0007). Stops at the first failure so the printed
label names the actual cause. The quality gate (`./uml crap` / `./uml mutate`) is
deliberately absent: it needs serialized, declared-scope runs against a real
project, so it cannot be a default check. Native LOVE checks open windows, so
they need the explicit `--native` opt-in.

Usage:
    python3 -I scripts/check.py            # pure checks only
    python3 -I scripts/check.py --native   # plus every real-window driver
"""
import argparse
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

REPO = Path(__file__).resolve().parents[1]


def interpreter(command):
    return shutil.which(command)


def run(argv, env=None, timeout=1800):
    result = subprocess.run([str(value) for value in argv], cwd=REPO, capture_output=True,
                            text=True, env=env, timeout=timeout)
    return result.returncode, (result.stdout or '') + (result.stderr or '')


def pure_suite(name, executable, probe):
    """A present interpreter that fails its probe is a misconfiguration, not a skip."""
    if not executable:
        return 'skipped', 0, f'{name} not on PATH'
    code, output = run([executable, '-e', probe])
    if code != 0:
        return 'failed', code, f'{name} identity probe failed: {output.strip()}'
    code, output = run([executable, str(REPO / 'scripts/run_busted.lua'), 'love/spec'])
    return ('executed' if code == 0 else 'failed'), code, output


def entries():
    python = interpreter('python3')
    if not python:
        return 'failed', 1, 'python3 is required by the installer and this suite'
    # The suite spawns the public entries; a rocks LUA_PATH must not leak in.
    env = {key: value for key, value in os.environ.items()
           if key not in ('LUA_PATH', 'LUA_CPATH', 'LUA_PATH_5_1', 'LUA_CPATH_5_1',
                          'LUA_PATH_5_4', 'LUA_CPATH_5_4')}
    code, output = run([python, '-I', str(REPO / 'scripts/test_posix_entries.py')], env=env)
    return ('executed' if code == 0 else 'failed'), code, output


def restart_entries():
    python = interpreter('python3')
    if not python:
        return 'failed', 1, 'python3 is required'
    code, output = run([python, '-I', str(REPO / 'scripts/test_posix_restart_entries.py')])
    return ('executed' if code == 0 else 'failed'), code, output


def native_driver(name, timeout):
    python = interpreter('python3')
    if not python:
        return 'skipped', 0, 'python3 not on PATH'
    if not interpreter(os.environ.get('UML_LOVE_EXECUTABLE', 'love')):
        return 'skipped', 0, 'LOVE not on PATH'
    code, output = run([python, '-I', str(REPO / 'scripts/run_love_driver.py'), name], timeout=timeout)
    if code == 0 and output.strip().startswith('SKIP'):
        # A driver whose precondition is absent (e.g. a font the installer has
        # not provisioned) verified nothing; it must not read as a pass.
        return 'skipped', code, output.strip()
    return ('executed' if code == 0 else 'failed'), code, output


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--native', action='store_true', help='also run the real-window drivers')
    parser.add_argument('--native-timeout', type=int, default=300)
    arguments = parser.parse_args()

    checks = [
        ('busted (Lua 5.4)', lambda: pure_suite('Lua 5.4', interpreter('lua5.4'),
                                                "assert(_VERSION == 'Lua 5.4'); print(_VERSION)")),
        ('busted (LuaJIT)', lambda: pure_suite('LuaJIT', interpreter('luajit'),
                                               "assert(jit and jit.version); print(jit.version)")),
        ('entries (python -I scripts/test_posix_entries.py)', entries),
        ('restart (python -I scripts/test_posix_restart_entries.py)', restart_entries),
    ]
    if arguments.native:
        sys.path.insert(0, str(REPO / 'scripts'))
        from run_love_driver import DRIVERS
        for name in sorted(DRIVERS):
            checks.append((f'native {name}',
                           lambda name=name: native_driver(name, arguments.native_timeout)))

    results = []
    lua_runs = 0
    for label, check in checks:
        status, code, output = check()
        results.append((label, status, code, output))
        if label.startswith('busted') and status != 'skipped':
            lua_runs += 1
        if status == 'failed':
            break
    # A run with no Lua interpreter at all verified nothing: fatal even though
    # each individual interpreter is only a skip.
    if lua_runs == 0:
        results.append(('lua interpreters', 'failed', 1,
                        'Neither lua5.4 nor luajit resolved; the pure suite did not run.'))

    counts = {'executed': 0, 'skipped': 0, 'failed': 0}
    for label, status, code, output in results:
        counts[status] = counts.get(status, 0) + 1
        prefix = {'executed': 'PASS', 'skipped': 'SKIP', 'failed': 'FAIL'}[status]
        print(f'{prefix} {label} (exit {code})')
        if status == 'skipped':
            print(f'     {output.strip()}')
        elif status == 'failed' and output:
            print(output.rstrip())
    print('note: ./uml crap and ./uml mutate are not run here; the quality gate needs '
          'serialized, declared-scope runs.')
    print(f"check: {counts['executed']} passed, {counts['skipped']} skipped, {counts['failed']} failed")
    return 1 if counts['failed'] else 0


if __name__ == '__main__':
    sys.exit(main())
