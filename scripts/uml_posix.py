"""POSIX 公共入口；安装与子进程始终使用字面 argv。"""
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import urllib.request
import zipfile

CIMGUI_ARCHIVE = 'eec63e283d80961240e22e7f869b66321addc0464d20b6ef69779f1b6759c18e'
CIMGUI_LIBRARY = '77465886fcbaa770da210062f95d236ea93f15dc9b97a50c3d1722b89db92611'
CIMGUI_URL = 'https://github.com/apicici/cimgui-love/releases/download/1.90-1/cimgui-love-linux-x64-1.90-1.zip'


def run(argv, **kwargs):
    return subprocess.run([str(x) for x in argv], check=True, **kwargs)


def host():
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise ValueError('此阶段仅支持 WSL x86_64；macOS Apple Silicon 安装尚待后续实现。')
    if 'microsoft' not in platform.release().lower():
        raise ValueError('Linux 执行路径仅交付 WSL2，请在 Ubuntu 24.04 WSL2 中运行。')


def love_command():
    executable = shutil.which(os.environ.get('UML_LOVE_EXECUTABLE', 'love'))
    if not executable or executable.lower().endswith(('.exe', '.ps1', '.cmd', '.bat')):
        raise ValueError('需要原生 LOVE 11.5：Ubuntu 中安装 love，或设置 UML_LOVE_EXECUTABLE。')
    result = run([executable, '--version'], capture_output=True, text=True)
    if not re.search(r'LOVE 11\.5\b', result.stdout):
        raise ValueError('需要 LOVE 11.5；实际版本：' + result.stdout.strip())
    return executable


def select_herdr():
    candidates = []
    reasons = []
    for directory in os.get_exec_path():
        path = Path(directory or '.') / 'herdr'
        if not path.is_file() or not os.access(path, os.X_OK):
            continue
        try:
            version = run([path, '--version'], capture_output=True, text=True, timeout=10).stdout.strip()
            match = re.fullmatch(r'herdr (\d+)\.(\d+)\.(\d+)', version)
            if not match or tuple(map(int, match.groups())) < (0, 9, 3):
                raise ValueError('需要纯数字版本 >= 0.9.3')
            schema = json.loads(run([path, 'api', 'schema', '--json'], capture_output=True,
                                    text=True, timeout=10).stdout)
            if schema.get('protocol') != 22:
                raise ValueError('需要 protocol 22')
            candidates.append((tuple(map(int, match.groups())), path.resolve()))
        except (ValueError, subprocess.SubprocessError) as error:
            reasons.append(f'{path}: {error}')
    if not candidates:
        raise ValueError('PATH 上没有兼容 Herdr >= 0.9.3 / protocol 22。\n' + '\n'.join(reasons))
    return max(candidates, key=lambda item: item[0])[1]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def atomic_write(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, name = tempfile.mkstemp(prefix=path.name + '.', dir=path.parent)
    try:
        with os.fdopen(fd, 'w') as stream:
            stream.write(data)
        os.replace(name, path)
    finally:
        Path(name).unlink(missing_ok=True)


def cimgui(directory):
    library = directory / 'cimgui.so'
    if not library.exists() or digest(library) != CIMGUI_LIBRARY or not (directory / 'cimgui/init.lua').is_file():
        # 下载、展开和执行目录分开；校验值来自 #107 实际资产记录，并非发布者签名。
        with tempfile.TemporaryDirectory(prefix='uml-cimgui-') as temporary:
            base = Path(temporary)
            download = base / 'download'
            expanded = base / 'expanded'
            download.mkdir()
            expanded.mkdir()
            archive = download / 'release.zip'
            urllib.request.urlretrieve(CIMGUI_URL, archive)
            if digest(archive) != CIMGUI_ARCHIVE:
                raise ValueError('cimgui-love archive SHA256 mismatch')
            with zipfile.ZipFile(archive) as bundle:
                for entry in bundle.infolist():
                    relative = Path(entry.filename)
                    if relative.is_absolute() or '..' in relative.parts or (entry.external_attr >> 16) & 0o170000 == 0o120000:
                        raise ValueError('cimgui-love archive contains unsafe paths')
                bundle.extractall(expanded)
            if digest(expanded / 'cimgui.so') != CIMGUI_LIBRARY or not (expanded / 'cimgui/init.lua').is_file():
                raise ValueError('cimgui-love release payload mismatch')
            directory.mkdir(parents=True, exist_ok=True)
            shutil.copy2(expanded / 'cimgui.so', library)
            shutil.copytree(expanded / 'cimgui', directory / 'cimgui', dirs_exist_ok=True)
    header = library.read_bytes()[:20]
    if header[:5] != b'\x7fELF\x02' or header[18:20] != b'\x3e\x00':
        raise ValueError('cimgui.so 必须是 ELF64 x86_64')
    dependencies = run(['ldd', library], capture_output=True, text=True)
    if 'not found' in dependencies.stdout:
        raise ValueError('cimgui.so 缺少运行依赖：\n' + dependencies.stdout)


def fetch(directory, tool):
    url = os.environ.get(('UML_VIEWER' if tool == 'uml-viewer' else tool.upper()) + '_REPO_URL',
                         f'https://github.com/qinyuanj/{tool}.git')
    ref = os.environ.get(('UML_VIEWER' if tool == 'uml-viewer' else tool.upper()) + '_REF', 'main')
    if directory.exists():
        if not (directory / '.git').exists():
            raise ValueError(f'拒绝替换非 Git 目录：{directory}')
        if run(['git', '-C', directory, 'status', '--porcelain'], capture_output=True, text=True).stdout:
            raise ValueError(f'安装副本有本地修改，已保留，请先处理：{directory}')
    else:
        run(['git', 'init', '-q', directory])
    run(['git', '-C', directory, 'fetch', '--depth', '1', url, ref])
    run(['git', '-C', directory, 'checkout', '-q', '--detach', 'FETCH_HEAD'])
    return {'source': url, 'ref': ref, 'commit': run(['git', '-C', directory, 'rev-parse', 'HEAD'],
                                                  capture_output=True, text=True).stdout.strip()}


def install(args):
    if args == ['--help'] or args == ['-h']:
        print('usage: get-uml-viewer [--install-only]')
        return 0
    if args not in ([], ['--install-only']):
        raise ValueError('usage: get-uml-viewer [--install-only]')
    host()
    love = love_command()
    herdr = select_herdr()
    root = Path.cwd().resolve()
    state = root / '.uml-viewer'
    state.mkdir(exist_ok=True)
    for name in ('companion.edn', 'recovery.edn', 'viewer-process-identity.json'):
        record = state / name
        if record.exists() and record.read_text().strip() not in ('', '{}', 'nil'):
            raise ValueError(f'保留已有运行态并拒绝安装：{record}；请先通过原有入口正常关闭会话并归档。')
    development = os.environ.get('UML_VIEWER_TOOL_ROOT')
    versions = {}
    if development:
        source = Path(development).resolve(strict=True)
        if not (source / 'love/main.lua').is_file() or not (source / 'scripts/uml_posix.py').is_file():
            raise ValueError('UML_VIEWER_TOOL_ROOT 必须指向完整开发仓库')
        destination = state / 'uml-viewer'
        if destination.exists() or destination.is_symlink():
            if not destination.is_symlink() or destination.resolve() != source:
                raise ValueError('开发源码与现有安装副本不同，拒绝替换。')
        else:
            destination.symlink_to(source, target_is_directory=True)
        versions['uml-viewer'] = {'source': str(source), 'development': True,
            'commit': run(['git', '-C', source, 'rev-parse', 'HEAD'], capture_output=True, text=True).stdout.strip(),
            'dirty': bool(run(['git', '-C', source, 'status', '--porcelain'], capture_output=True, text=True).stdout)}
    else:
        versions['uml-viewer'] = fetch(state / 'uml-viewer', 'uml-viewer')
    for tool in ('crapper', 'mutator', 'dryer'):
        versions[tool] = fetch(state / tool, tool)
    cimgui(state / 'cimgui-love-1.90-1')
    for tool in ('crapper', 'mutator', 'dryer'):
        directory = state / tool
        python = directory / '.venv/bin/python'
        if not python.exists():
            run([sys.executable, '-I', '-m', 'venv', directory / '.venv'])
        packages = ['-e', str(directory)]
        if tool == 'mutator':
            packages += ['-e', str(state / 'crapper')]
        run([python, '-I', '-m', 'pip', 'install', '--disable-pip-version-check', *packages])
        run([python, '-I', '-m', 'pip', 'check'])
    ignore = root / '.gitignore'
    text = ignore.read_text() if ignore.exists() else ''
    text = re.sub(r'(?ms)^# BEGIN UML-VIEWER\n.*?^# END UML-VIEWER\n?', '', text)
    atomic_write(ignore, text.rstrip() + '\n# BEGIN UML-VIEWER\n.uml-viewer/\numl-viewer-log.txt\n# END UML-VIEWER\n')
    entry = root / 'uml'
    if entry.exists() and entry.read_bytes() != (state / 'uml-viewer/scripts/uml').read_bytes():
        if b'Thin Unix entry' not in entry.read_bytes():
            raise ValueError('已有自定义 ./uml，拒绝覆盖；请先改名保存。')
    shutil.copy2(state / 'uml-viewer/scripts/uml', entry)
    entry.chmod(0o755)
    atomic_write(state / 'install.json', json.dumps({'version': 1, 'projectRoot': str(root),
                 'love': love, 'herdr': str(herdr), 'repositories': versions}, ensure_ascii=False, indent=2) + '\n')
    print('已安装 ./uml；使用 ./uml ir 生成 EDN。')
    return 0 if args else command(root, [])


def command(root, args):
    root = root.resolve()
    os.chdir(root)
    if args in (['--help'], ['-h']):
        print('usage: uml [--runtime love] [--backend herdr] [--restart | ir | crap | mutate | dry] [args]')
        return 0
    host()
    if '--runtime' in args:
        index = args.index('--runtime')
        if args[index:index + 2] != ['--runtime', 'love'] or args.count('--runtime') != 1:
            raise ValueError('--runtime 仅接受 love；JVM 已移除。')
        args = args[:index] + args[index + 2:]
    tool_root = Path(os.environ.get('UML_VIEWER_TOOL_ROOT', root / '.uml-viewer/uml-viewer')).resolve()
    if not (tool_root / 'love/main.lua').is_file():
        raise ValueError('缺少 viewer 安装副本；请运行 get-uml-viewer，或设置 UML_VIEWER_TOOL_ROOT 指向开发仓库。')
    if args and args[0] == 'ir':
        values = args[1:]
        has_policy = '--policy' in values
        index = 0
        while index < len(values):
            if values[index] in ('--root', '--output', '--policy'):
                index += 2
            else:
                has_policy = has_policy or not values[index].startswith('--')
                index += 1
        if not has_policy:
            preferred = root / 'examples' / (root.name + '.policy.edn')
            choices = sorted((root / 'examples').glob('*.policy.edn'))
            policy = preferred if preferred.is_file() else choices[0] if choices else root / 'policy.edn'
            if not policy.is_file():
                raise ValueError('没有 policy；请创建 policy.edn 或 examples/*.policy.edn。')
            args = ['ir', str(policy.relative_to(root)), *args[1:]]
        return subprocess.call([love_command(), str(tool_root / 'love'), *args], cwd=root)
    if args and args[0] in ('crap', 'mutate', 'dry'):
        # 质量分发实现与安装器共用入口，独立模块保留协议边界。
        from uml_quality import dispatch
        return dispatch(root, args[0], args[1:])
    # 其余路径都是 viewer 生命周期：默认 fresh、--restart 替换、--backend herdr。
    from uml_launch import launch
    return launch(root, tool_root, args, restart='--restart' in args)


def main():
    try:
        if sys.version_info < (3, 11):
            raise ValueError('需要 Python 3.11+')
        if sys.argv[1] == 'install':
            return install(sys.argv[2:])
        return command(Path(sys.argv[2]), sys.argv[3:])
    except subprocess.CalledProcessError as error:
        print(f'uml: 命令失败（退出 {error.returncode}）：{error.cmd[0]}', file=sys.stderr)
        return error.returncode if error.returncode > 0 else 128 - error.returncode
    except (ValueError, OSError, subprocess.TimeoutExpired) as error:
        print(f'uml: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    # -I 排除项目目录；只加入受信任的入口脚本目录。
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    sys.exit(main())
