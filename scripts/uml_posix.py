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
import tomllib
import urllib.request
import zipfile

CIMGUI_ARCHIVE = 'eec63e283d80961240e22e7f869b66321addc0464d20b6ef69779f1b6759c18e'
CIMGUI_LIBRARY = '77465886fcbaa770da210062f95d236ea93f15dc9b97a50c3d1722b89db92611'
CIMGUI_URL = 'https://github.com/apicici/cimgui-love/releases/download/1.90-1/cimgui-love-linux-x64-1.90-1.zip'
# The viewer's only UI font (#82). These are the same archive and face hashes
# the retired PowerShell installer pinned; only the extraction path changed.
FONT_URL = ('https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.42/'
            'SarasaMonoSC-TTF-1.0.42.7z')
FONT_FACE = 'SarasaMonoSC-Regular.ttf'
FONT_ARCHIVE_SHA256 = 'aa2150e99eb38c5f9d3a00fe58e3f90a9d89495c795a8ac80934d1c3e6c377ee'
FONT_SHA256 = '82e09460f6f55010296f670b85e346ddeaf91b967724840d2bf5514137f68a83'
# Upstream ships the TTF release only as 7z, and no 7z binary is assumed on the
# host. py7zr lives in the installer's own environment so no tool venv is
# polluted with an unrelated dependency.
PY7ZR = 'py7zr==1.1.4'


def run(argv, **kwargs):
    kwargs.setdefault('check', True)
    return subprocess.run([str(x) for x in argv], **kwargs)


def host():
    if platform.system() != 'Linux' or platform.machine() != 'x86_64':
        raise ValueError('仅支持 WSL2 x86_64（ADR 0007），请在 Ubuntu 24.04 WSL2 中运行。')
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


def git_command():
    """The installer fetches every tool repository with a literal `git`."""
    executable = shutil.which('git')
    if not executable:
        raise ValueError('需要 git（安装器用它获取工具仓库）：sudo apt install git。')
    return executable


def cargo_command():
    """Locate a runnable cargo; the pinned version is checked per checkout."""
    executable = shutil.which(os.environ.get('UML_CARGO_EXECUTABLE', 'cargo'))
    if not executable:
        raise ValueError('需要 Rust 工具链（cargo）：用 rustup 安装（https://rustup.rs），'
                         '或设置 UML_CARGO_EXECUTABLE。所需版本见工具仓库的 rust-toolchain.toml。')
    try:
        result = subprocess.run([executable, '--version'], capture_output=True, text=True)
    except OSError as error:
        raise ValueError(f'cargo 不可执行：{executable}（{error}）') from error
    if result.returncode != 0 or not re.match(r'cargo \d+\.\d+\.\d+', result.stdout):
        raise ValueError(f'cargo 不可用：{executable} --version 输出 '
                         f'{(result.stdout or result.stderr).strip()!r}')
    return executable


def rust_channel(tool_root):
    """The toolchain pin every build must honour (#134): rust-toolchain.toml."""
    pin = Path(tool_root) / 'rust-toolchain.toml'
    if not pin.is_file():
        raise ValueError(f'工具仓库缺少 rust-toolchain.toml：{pin}；安装副本不完整。')
    channel = tomllib.loads(pin.read_text()).get('toolchain', {}).get('channel')
    if not channel:
        raise ValueError(f'{pin} 没有 toolchain.channel；安装副本不完整。')
    return channel


def check_rust_toolchain(cargo, tool_root):
    """The installed toolchain must match the checkout's pin.

    A rustup shim resolves rust-toolchain.toml per directory, so the probe
    runs inside the checkout; a plain distro cargo must match on its own.
    """
    channel = rust_channel(tool_root)
    result = subprocess.run([cargo, '--version'], cwd=tool_root,
                            capture_output=True, text=True)
    match = re.match(r'cargo (\d+\.\d+\.\d+)', result.stdout or '')
    if result.returncode != 0 or not match:
        raise ValueError(f'无法确认 Rust 工具链（rust-toolchain.toml 要求 {channel}）：'
                         f'{(result.stderr or result.stdout).strip()}')
    if re.fullmatch(r'\d+\.\d+\.\d+', channel) and match.group(1) != channel:
        raise ValueError(f'Rust 工具链版本不符：rust-toolchain.toml 要求 {channel}，'
                         f'实际 {match.group(1)}；rustup toolchain install {channel}。')
    return channel


def rust_build(cargo, tool_root):
    """Build the single uml-viewer binary the entry dispatch expects (#143)."""
    check_rust_toolchain(cargo, tool_root)
    run([cargo, 'build', '--release', '--locked'], cwd=tool_root)
    binary = Path(tool_root) / 'target/release/uml-viewer'
    if not (binary.is_file() and os.access(binary, os.X_OK)):
        raise ValueError(f'cargo build 未产出可执行文件 {binary}')
    return binary


def x11_library_present():
    """Probe libxkbcommon-x11 without elevating (mirrors the viewer's)."""
    name = 'libxkbcommon-x11.so.0'
    if any((Path(directory) / name).exists() for directory in
           ('/usr/lib/x86_64-linux-gnu', '/usr/lib/aarch64-linux-gnu', '/usr/lib')):
        return True
    result = subprocess.run(['ldconfig', '-p'], capture_output=True, text=True)
    return name in result.stdout


def x11_warnings(present=None):
    """X11 libs are optional: #139 chose the Wayland-undecorated window on the
    delivery host, so an absent libxkbcommon-x11 is a hint, never a refusal."""
    if present is None:
        present = x11_library_present()
    if present:
        return []
    return ['未找到 libxkbcommon-x11.so.0：viewer 将使用 Wayland 无装饰窗口（#139 已实测）；'
            '需要 X11 标题栏可安装 libxkbcommon-x11-0（sudo apt install libxkbcommon-x11-0）。']


# Both implementations coexist until parity (#134); love stays the default.
RUNTIMES = ('love', 'rust')


def take_runtime(args):
    """Strip one `--runtime love|rust` pair; refuse anything else.

    `UML_RUNTIME` supplies the default so the existing suites can be pointed
    at either runtime without changing their own argv (#138): an explicit
    `--runtime` still wins, and an invalid value is refused the same way.
    """
    requested = os.environ.get('UML_RUNTIME')
    if requested is not None and requested not in RUNTIMES:
        raise ValueError('--runtime 只接受 love 或 rust（默认 love）；JVM 已移除。')
    default = requested or 'love'
    if '--runtime' not in args:
        return default, args
    index = args.index('--runtime')
    value = args[index + 1] if index + 1 < len(args) else None
    if value not in RUNTIMES or args.count('--runtime') != 1:
        raise ValueError('--runtime 只接受 love 或 rust（默认 love）；JVM 已移除。')
    return value, args[:index] + args[index + 2:]


def rust_command(tool_root):
    configured = os.environ.get('UML_VIEWER_RUST_EXECUTABLE')
    executable = Path(configured) if configured else Path(tool_root) / 'target/release/uml-viewer'
    if not (executable.is_file() and os.access(executable, os.X_OK)):
        raise ValueError(f'缺少 Rust 二进制 {executable}；请运行 get-uml-viewer 构建（或在 {tool_root} '
                         '运行 cargo build --release --locked），或设置 UML_VIEWER_RUST_EXECUTABLE。')
    return str(executable)


def runtime_argv(runtime, tool_root):
    """argv prefix that precedes a subcommand (`viewer`, `ir`, …) for a runtime."""
    if runtime == 'rust':
        return [rust_command(tool_root)]
    # LOVE takes the game directory first, then the game's own arguments.
    return [love_command(), str(Path(tool_root) / 'love')]


# Quality tools analyze source as Lua 5.4 and require the pinned cluacov that
# LuaCov loads, so the preflight rejects any other interpreter or cluacov.
LUA_PROBE = ('pcall(require, "luarocks.loader"); require("lfs"); require("luacov"); '
             'local ok, v = pcall(require, "cluacov.version"); '
             'if not ok then v = "" end; '
             'io.write(_VERSION .. " " .. tostring(v)); '
             'if _VERSION ~= "Lua 5.4" or v ~= "1.0.0" then os.exit(1) end')


def lua_command():
    requested = os.environ.get('UML_LUA_EXECUTABLE')
    found = []
    for name in ([requested] if requested else ['lua5.4', 'lua']):
        executable = shutil.which(name)
        if not executable:
            continue
        result = run([executable, '-e', LUA_PROBE], capture_output=True, text=True, check=False)
        if result.returncode == 0:
            return executable
        found.append(f'{executable}: {(result.stdout or result.stderr).strip() or "exit " + str(result.returncode)}')
    raise ValueError('需要 Lua 5.4 及 lfs、luacov、cluacov 1.0.0（质量工具按 Lua 5.4 分析源码）：'
                     'sudo apt install lua5.4 lua-filesystem lua-luacov && '
                     'luarocks --local --lua-version=5.4 install cluacov 1.0.0-1，或设置 UML_LUA_EXECUTABLE。'
                     + ('实际：' + '；'.join(found) if found else ''))


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


# Paths this installer creates inside a tool checkout. They must not read as
# the user's own work, otherwise the second install would refuse its own venv.
# Filtering happens here rather than in a git pathspec: glob semantics differ
# between git versions and a mismatch silently reintroduces the false refusal.
PROVISIONED_DIRECTORIES = ('.venv', '__pycache__', '.pytest_cache')


EXTRACT = """
import pathlib, sys
import py7zr
archive, target, face = pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2]), sys.argv[3]
with py7zr.SevenZipFile(archive, 'r') as bundle:
    names = [n for n in bundle.getnames()
             if pathlib.Path(n).name == face and not pathlib.Path(n).is_absolute()
             and '..' not in pathlib.Path(n).parts]
    if len(names) != 1:
        sys.exit(f'Sarasa archive must contain exactly one {face}, found {len(names)}')
    bundle.extract(path=target, targets=names)
    print(names[0])
"""


def extractor(directory):
    """An interpreter that can read 7z: the host's, else the installer's own.

    py7zr is the only dependency the installer needs beyond the stdlib; using
    the host interpreter when it already has the module avoids creating a venv
    for nothing.
    """
    probe = subprocess.run([sys.executable, '-I', '-c', 'import py7zr'],
                           capture_output=True, text=True)
    if probe.returncode == 0:
        return sys.executable
    override = os.environ.get('UML_VIEWER_FONT_EXTRACTOR')
    if override:
        return override
    environment = directory.parent / 'installer'
    python = environment / '.venv/bin/python'
    if not python.is_file():
        run([sys.executable, '-I', '-m', 'venv', environment / '.venv'])
        run([str(python), '-I', '-m', 'pip', 'install', '--disable-pip-version-check',
             '--quiet', PY7ZR])
    return str(python)


def font(directory):
    """Provision the pinned Sarasa face, verified by SHA256.

    Hashes come from #82 (the archive and the face), so a substituted download
    cannot reach the viewer. Upstream publishes the TTF release only as 7z, so
    extraction happens in an interpreter that has py7zr. The three
    UML_VIEWER_FONT_* variables override the pin for fixture tests, exactly as
    the retired installer's -Pin parameter did.
    """
    url = os.environ.get('UML_VIEWER_FONT_URL', FONT_URL)
    archive_hash = os.environ.get('UML_VIEWER_FONT_ARCHIVE_SHA256', FONT_ARCHIVE_SHA256)
    face_hash = os.environ.get('UML_VIEWER_FONT_SHA256', FONT_SHA256)
    target = directory / FONT_FACE
    if target.is_file() and digest(target) == face_hash:
        return
    python = extractor(directory)
    with tempfile.TemporaryDirectory(prefix='uml-sarasa-') as temporary:
        base = Path(temporary)
        archive = base / url.rsplit('/', 1)[-1]
        urllib.request.urlretrieve(url, archive)
        if digest(archive) != archive_hash:
            raise ValueError('Sarasa font archive SHA256 mismatch')
        extracted = run([python, '-I', '-c', EXTRACT, str(archive),
                         str(base / 'expanded'), FONT_FACE], capture_output=True, text=True)
        if extracted.returncode != 0:
            raise ValueError(extracted.stderr.strip() or 'Sarasa font archive extraction failed')
        face = base / 'expanded' / extracted.stdout.strip()
        if not face.is_file() or digest(face) != face_hash:
            raise ValueError(f'{FONT_FACE} SHA256 mismatch')
        directory.mkdir(parents=True, exist_ok=True)
        shutil.copy2(face, target)


def provisioned(path):
    name = path.rstrip('/').rsplit('/', 1)[-1]
    return name in PROVISIONED_DIRECTORIES or name.endswith('.egg-info')


def local_changes(directory):
    """The checkout's uncommitted work, excluding what this installer creates."""
    result = run(['git', '-C', directory, 'status', '--porcelain', '-z'],
                 capture_output=True, text=True)
    changes = []
    for entry in result.stdout.split('\0'):
        if not entry:
            continue
        # Renames carry the original path in the following NUL-separated field.
        if provisioned(entry[3:]):
            continue
        changes.append(entry)
    return '\n'.join(changes)


def fetch(directory, tool):
    url = os.environ.get(('UML_VIEWER' if tool == 'uml-viewer' else tool.upper()) + '_REPO_URL',
                         f'https://github.com/qinyuanj/{tool}.git')
    ref = os.environ.get(('UML_VIEWER' if tool == 'uml-viewer' else tool.upper()) + '_REF', 'main')
    if directory.exists():
        if not (directory / '.git').exists():
            raise ValueError(f'拒绝替换非 Git 目录：{directory}')
        if local_changes(directory):
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
    git_command()
    love = love_command()
    lua_command()
    herdr = select_herdr()
    cargo = cargo_command()
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
            'dirty': bool(local_changes(source))}
    else:
        versions['uml-viewer'] = fetch(state / 'uml-viewer', 'uml-viewer')
    for tool in ('crapper', 'mutator', 'dryer'):
        versions[tool] = fetch(state / tool, tool)
    cimgui(state / 'cimgui-love-1.90-1')
    # Provisioned after the tool environments exist: extraction uses the
    # installer's own venv, which Python 3.12 provides without system packages.
    font(state / 'fonts')
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
    # The Rust build comes last: the checkout must be complete, and a refusal
    # in any earlier step must not leave a half-built tree behind. `target/`
    # is gitignored, so a rebuild never reads as a local modification.
    binary = rust_build(cargo, state / 'uml-viewer')
    for warning in x11_warnings():
        print(f'提示：{warning}', file=sys.stderr)
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
                 'love': love, 'rust': str(binary), 'herdr': str(herdr), 'repositories': versions}, ensure_ascii=False, indent=2) + '\n')
    print('已安装 ./uml；使用 ./uml ir 生成 EDN。')
    return 0 if args else command(root, [])


def command(root, args):
    root = root.resolve()
    os.chdir(root)
    if args in (['--help'], ['-h']):
        print('usage: uml [--runtime love|rust] [--backend herdr] [--restart | ir | scene | crap | mutate | dry] [args]\n'
              '  scene 仅用于 --runtime rust。')
        return 0
    host()
    runtime, args = take_runtime(args)
    tool_root = Path(os.environ.get('UML_VIEWER_TOOL_ROOT', root / '.uml-viewer/uml-viewer')).resolve()
    if runtime == 'love' and not (tool_root / 'love/main.lua').is_file():
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
        return subprocess.call([*runtime_argv(runtime, tool_root), *args], cwd=root)
    if runtime == 'rust' and args and args[0] == 'scene':
        # Headless scene export exists only in the Rust binary (#134).
        return subprocess.call([*runtime_argv(runtime, tool_root), *args], cwd=root)
    if args and args[0] in ('crap', 'mutate', 'dry'):
        # 质量分发实现与安装器共用入口，独立模块保留协议边界。
        from uml_quality import dispatch
        return dispatch(root, args[0], args[1:])
    # 其余路径都是 viewer 生命周期：默认 fresh、--restart 替换、--backend herdr。
    from uml_launch import launch
    return launch(root, tool_root, args, restart='--restart' in args, runtime=runtime)


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
