#!/usr/bin/env python3
"""#114 harness helper. Observes; never decides for the companion.

usage: h.py <run-dir> trigger <op> <class-id>
       h.py <run-dir> context real | proposal <id> <name>
       h.py <run-dir> wait-agent-empty [timeout]
       h.py <run-dir> status | pane <file> | transcript <session-id> [out]
"""
import json
import re
import subprocess
import sys
import time
from pathlib import Path

PROJECT = Path('/tmp/uml-114/库存 项目')
run = Path(sys.argv[1])
triggers = run / 'triggers'
cmd = sys.argv[2]


def owned():
    text = (PROJECT / '.uml-viewer/companion.edn').read_text()
    session = re.search(r':session "([^"]+)"', text).group(1)
    pane = re.search(r':pane "([^"]+)"', text).group(1)
    return session, pane


def next_index():
    n = 1
    while (triggers / f'{n}.edn').exists() or (triggers / f'{n}.edn.done').exists():
        n += 1
    return n


def drop(text):
    n = next_index()
    tmp = triggers / f'{n}.tmp'
    tmp.write_text(text)
    tmp.rename(triggers / f'{n}.edn')
    deadline = time.monotonic() + 30
    while not (triggers / f'{n}.edn.done').exists():
        if time.monotonic() > deadline:
            sys.exit(f'trigger {n} not consumed by viewer')
        time.sleep(0.2)
    print(f'trigger {n} consumed at {time.strftime("%H:%M:%S")}')
    print((PROJECT / '.uml-viewer/to-agent.edn').read_text())


if cmd == 'trigger':
    drop('{:op :%s :id :%s}' % (sys.argv[3], sys.argv[4]))
elif cmd == 'context':
    if sys.argv[3] == 'real':
        drop('{:op :context :extra {:context :real}}')
    else:
        drop('{:op :context :extra {:context :proposal :proposal-id :%s :name "%s"}}' % (sys.argv[4], sys.argv[5]))
elif cmd == 'wait-agent-empty':
    timeout = float(sys.argv[3]) if len(sys.argv) > 3 else 600
    deadline = time.monotonic() + timeout
    while ':queue []' not in (PROJECT / '.uml-viewer/to-agent.edn').read_text():
        if time.monotonic() > deadline:
            sys.exit('to-agent queue not drained')
        time.sleep(1)
    print('to-agent drained at', time.strftime('%H:%M:%S'))
elif cmd == 'status':
    print((triggers / 'status.txt').read_text())
    print('to-agent:', (PROJECT / '.uml-viewer/to-agent.edn').read_text())
    print('to-viewer:', (PROJECT / '.uml-viewer/to-viewer.edn').read_text())
    result = PROJECT / '.uml-viewer/quality-result.json'
    print('quality-result:', result.read_text() if result.exists() else '(absent)')
elif cmd == 'pane':
    session, pane = owned()
    out = subprocess.run(['herdr', '--session', session, 'pane', 'read', pane, '--lines', '400'],
                         capture_output=True, text=True)
    if out.returncode != 0:
        out = subprocess.run(['herdr', '--session', session, 'pane', 'read', pane], capture_output=True, text=True)
    text = '\n'.join(line.rstrip() for line in out.stdout.splitlines())
    (run / sys.argv[3]).write_text(text + '\n')
    print('\n'.join(l for l in text.splitlines() if l.strip())[-6000:])
elif cmd == 'transcript':
    session_id = sys.argv[3]
    path = next(Path.home().glob(f'.claude/projects/*/{session_id}.jsonl'))
    lines = []
    pending = {}
    for raw in path.read_text().splitlines():
        entry = json.loads(raw)
        message = entry.get('message') or {}
        content = message.get('content')
        stamp = entry.get('timestamp', '')
        if entry.get('type') == 'user' and isinstance(content, str):
            lines.append(f'{stamp} USER {content[:300]!r}')
        if not isinstance(content, list):
            continue
        for block in content:
            kind = block.get('type')
            if kind == 'text' and entry.get('type') == 'user':
                lines.append(f'{stamp} USER {block["text"][:300]!r}')
            elif kind == 'text':
                lines.append(f'{stamp} CLAUDE {block["text"][:1200]}')
            elif kind == 'tool_use':
                command = block.get('input', {}).get('command') or json.dumps(block.get('input'), ensure_ascii=False)[:400]
                lines.append(f'{stamp} TOOL {block.get("name")} {command[:1500]}')
            elif kind == 'tool_result':
                body = block.get('content')
                if isinstance(body, list):
                    body = ' '.join(b.get('text', '') for b in body if isinstance(b, dict))
                body = str(body)
                flag = ' ERROR' if block.get('is_error') else ''
                lines.append(f'{stamp} RESULT{flag} {body[-1500:]}')
        if message.get('model') and entry.get('type') == 'assistant':
            pending['model'] = message['model']
    lines.append(f'model: {pending.get("model")}')
    text = '\n'.join(lines) + '\n'
    if len(sys.argv) > 4:
        (run / sys.argv[4]).write_text(text)
    print(text[-12000:])
