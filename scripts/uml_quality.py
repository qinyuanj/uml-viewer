"""POSIX 质量分发：声明 scope、串行执行和原子结果记录。"""
import json
import os
from pathlib import Path
import re
import subprocess
import sys

from uml_posix import atomic_write

TOOLS = {'crap': ('crapper', [0, 2]), 'mutate': ('mutator', [0, 3]), 'dry': ('dryer', [0])}
SKIP = {'.git', '.uml-viewer', '.venv', 'test', 'tests', 'spec', 'specs', 'vendor', 'node_modules', 'target', '__pycache__'}
EXTENSIONS = {'.clj', '.cljc', '.cljs', '.cljd', '.bb', '.java', '.go', '.ts', '.tsx', '.mts', '.cts', '.rs', '.py', '.lua'}
VALUES = {'--coverage-command', '--threshold', '--test-command', '--busted-task', '--timeout-factor', '--mutation-warning', '--max-workers', '--lines', '--min-lines', '--min-nodes', '--format'}
ROUTING = {'--root', '--source-root', '-s', '--exclude', '--full-tree', '--changed', '--mutate-all', '--edn', '--text'}


def strings(value):
    return isinstance(value, list) and all(isinstance(x, str) and x.strip() for x in value)


def read_config(root):
    path = root / 'uml-quality.json'
    if not path.exists():
        return {}
    config = json.loads(path.read_text())
    if not isinstance(config, dict) or type(config.get('version')) is not int or config['version'] != 1 or not {'standardTools', 'operations'} & config.keys() or 'aliasResultExitCodes' in config:
        raise ValueError('uml-quality.json 需要 version 1 和 standardTools 或 operations；Clojure alias 已移除。')
    if 'standardTools' in config:
        standard = config['standardTools']
        if not isinstance(standard, dict):
            raise ValueError('standardTools 必须是对象')
        for key, value in standard.items():
            if key not in {'sourceRoots', 'exclude', 'crapperOptions', 'mutatorOptions', 'dryerOptions'} or not strings(value):
                raise ValueError(f'无效 standardTools.{key}')
            if key.endswith('Options'):
                pending = False
                for option in value:
                    if pending:
                        pending = False
                    elif option in ROUTING or not option.startswith('--'):
                        raise ValueError(f'{key} 不允许 scope 或位置参数：{option}')
                    else:
                        pending = option in VALUES
                if pending:
                    raise ValueError(f'{key} 缺少参数值')
    if 'operations' in config:
        operations = config['operations']
        if not isinstance(operations, dict) or not {'crap', 'mutate'} <= operations.keys():
            raise ValueError('operations 必须声明 crap 和 mutate')
        for name in ('crap', 'mutate', 'dry'):
            if name not in operations:
                continue
            op = operations[name]
            if not isinstance(op, dict) or not isinstance(op.get('argv'), list) or not op['argv'] or not all(isinstance(x, str) for x in op['argv']) or not op['argv'][0].strip() or op.get('forwardArgs') not in ('append', 'none'):
                raise ValueError(f'无效 operations.{name}')
            codes = op.get('validResultExitCodes')
            if not isinstance(codes, list) or not codes or any(type(c) is not int or not 0 <= c <= 255 for c in codes):
                raise ValueError(f'{name} 需要 0–255 的 validResultExitCodes')
            if name == 'crap' and (op.get('scope') not in ('targets', 'whole-tree') or op['scope'] == 'whole-tree' and op['forwardArgs'] != 'none'):
                raise ValueError('whole-tree CRAP 必须 forwardArgs none；scope 仅接受 targets/whole-tree')
    return config


def scope(root, standard):
    roots = []
    for name in standard.get('sourceRoots', ['.']):
        if Path(name).is_absolute() or '..' in Path(name).parts:
            raise ValueError(f'业务源码根目录必须是项目内相对路径：{name}')
        path = (root / name).resolve()
        if not path.is_relative_to(root) or not path.is_dir():
            raise ValueError(f'无效业务源码根目录：{name}')
        roots.append(path)
    for name in standard.get('exclude', []):
        if Path(name).is_absolute() or '..' in Path(name).parts:
            raise ValueError(f'无效 exclude：{name}')
    return roots or [root]


def excluded(relative, patterns):
    for pattern in patterns:
        segments = pattern.removeprefix('./').rstrip('/').split('/')
        parts = []
        for index, segment in enumerate(segments):
            if segment == '**':
                parts.append('.*' if index == len(segments) - 1 else '(?:[^/]+/)*')
            else:
                parts.append(re.escape(segment).replace(r'\*', '[^/]*').replace(r'\?', '[^/]'))
                if index < len(segments) - 1:
                    parts.append('/')
        if re.match('^' + ''.join(parts) + '(?:/.*)?$', relative):
            return True
    return False


def business(root, path, roots, exclusions):
    canonical = path.resolve()
    return canonical.is_relative_to(root) and any(canonical.is_relative_to(p) for p in roots) and not excluded(path.relative_to(root).as_posix(), exclusions)


def targets(root, forward, roots, exclusions):
    options, named = [], []  # (name, named by the caller)
    index = 0
    while index < len(forward):
        arg = forward[index]
        if arg == '--changed':
            for command in (['diff', '--name-only', '-z', '--diff-filter=AM', 'HEAD'], ['ls-files', '--others', '--exclude-standard', '-z']):
                result = subprocess.run(['git', '-C', str(root), *command], capture_output=True, check=True)
                named += [(os.fsdecode(x), False) for x in result.stdout.split(b'\0') if x]
        elif arg.startswith('-'):
            if arg in ('--root', '--source-root', '-s', '--exclude'):
                raise ValueError('mutation scope 由 standardTools 声明')
            options.append(arg)
            if arg in VALUES:
                index += 1
                if index == len(forward):
                    raise ValueError(f'{arg} 缺少参数值')
                options.append(forward[index])
        else:
            named.append((arg, True))
        index += 1
    files = []
    for name, explicit in named:
        path = root / name
        if path.is_symlink():
            raise ValueError(f'mutation 不允许符号链接：{name}')
        candidates = sorted(path.rglob('*')) if path.is_dir() else [path]
        for file in candidates:
            if file.is_symlink():
                raise ValueError(f'mutation 不允许符号链接：{file}')
            if not file.is_file():
                continue
            if not file.resolve().is_relative_to(root):
                raise ValueError(f'mutation 目标位于项目之外：{file}')
            relative = file.relative_to(root)
            if file.suffix not in EXTENSIONS or SKIP.intersection(relative.parts):
                continue
            if not business(root, file, roots, exclusions):
                if explicit and file == path:
                    print(f'uml: omitted mutation target outside business scope: {relative.as_posix()}', file=sys.stderr)
            elif str(relative) not in files:
                files.append(str(relative))
    if not files:
        raise ValueError('没有业务 mutation 目标；请提供明确文件或 --changed。')
    return options, files


def dispatch(root, operation, forward):
    result_path = root / '.uml-viewer/quality-result.json'
    result_path.unlink(missing_ok=True)
    config = read_config(root)
    standard = config.get('standardTools', {})
    roots = scope(root, standard)
    tool, codes = TOOLS[operation]
    override = config.get('operations', {}).get(operation)
    scope_name = None
    if override:
        argv = override['argv'] + (forward if override['forwardArgs'] == 'append' else [])
        if argv[0].lower().endswith(('.ps1', '.exe', '.bat', '.cmd')):
            raise ValueError('POSIX 质量命令需要原生可执行入口')
        commands = [argv]
        codes = override['validResultExitCodes']
        scope_name = override.get('scope') if operation == 'crap' else None
    else:
        if standard and any(x in ('--root', '--source-root', '-s', '--exclude') for x in forward):
            raise ValueError('业务 scope 已由 standardTools 声明，不能用 refresh 参数覆盖')
        directory = root / '.uml-viewer' / tool
        python = directory / '.venv/bin/python'
        if not python.is_file():
            raise ValueError(f'缺少 {python}；请重新运行 get-uml-viewer。')
        argv = [str(python), '-m', tool]
        options = standard.get(tool + 'Options', [])
        if operation == 'crap':
            scope_args = [value for source in standard.get('sourceRoots', []) for value in ('--source-root', source)]
            scope_args += [value for exclude in standard.get('exclude', []) for value in ('--exclude', exclude)]
            commands = [argv + ['--full-tree'] + scope_args + options + forward]
            scope_name = 'whole-tree'
        elif operation == 'mutate':
            extra, files = targets(root, forward, roots, standard.get('exclude', []))
            commands = [argv + options + extra + files]
        else:
            if standard.get('exclude'):
                files = sorted({str(p.relative_to(root)) for source in roots for p in source.rglob('*') if p.is_file() and not p.is_symlink() and p.suffix in EXTENSIONS and business(root, p, roots, standard['exclude'])})
                if not files:
                    raise ValueError('没有重复检测目标')
                scope_args = files
            else:
                scope_args = [value for source in standard.get('sourceRoots', []) for value in ('-s', source)]
            commands = [argv + scope_args + options + forward]
    valid = True
    code = 0
    for argv in commands:
        actual = subprocess.call(argv, cwd=root)
        actual = actual if actual >= 0 else 128 - actual
        valid = valid and actual in codes
        if actual or not code:
            code = actual
        if actual not in codes:
            break
    record = {'version': 1, 'operation': operation, 'projectRoot': str(root), 'exitCode': code,
              'validResult': valid, 'crapScope': scope_name, 'configured': bool(override),
              'tool': 'configured' if override else tool}
    try:
        atomic_write(result_path, json.dumps(record) + '\n')
    except OSError as error:
        print(f'uml: 无法发布质量结果记录：{error}', file=sys.stderr)
    return code
