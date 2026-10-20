# #106 WSL 验收记录

日期：2026-10-21。原生 Ubuntu 24.04 WSL2 + WSLg，非 Windows 二进制、无 PowerShell。

## 实际环境

| 组件 | 版本/来源 |
|---|---|
| 内核 | `Linux 6.18.33.2-microsoft-standard-WSL2`，x86_64 |
| 显示 | WSLg（Weston 9 + Xwayland），`DISPLAY=:0` |
| LOVE | 发行版包 `love 11.5`（`/usr/bin/love`） |
| Lua | `lua5.1`（默认）、`lua5.4 5.4.6`、`LuaJIT 2.1` |
| cimgui | 仓库内 `cimgui-love 1.90-1`，ELF64 x86_64，`ldd` 无缺失 |
| Herdr | `0.9.5`，bundled protocol 22 |
| Python | 系统 `3.12.3`（工具 venv 各自独立） |

## 自动断言（可复现）

| 检查 | 结果 | 命令 |
|---|---|---|
| 双解释器纯逻辑 | 679 / 679 通过（Lua 5.4、LuaJIT；`lua` 5.1 亦通过） | `lua5.4|luajit scripts/run_busted.lua` |
| 全量 POSIX 检查 | 4 / 4 通过 | `python3 -I scripts/check.py` |
| POSIX 原生进程行为 | 12 / 12 通过 | `luajit scripts/run_posix_native_busted.lua` |
| 公共入口（安装/IR/质量） | 14 / 14 通过 | `python3 -I scripts/test_posix_entries.py` |
| restart 拒绝规则 | 4 / 4 通过 | `python3 -I scripts/test_posix_restart_entries.py` |
| viewer 渲染与重载 | 通过，含亚秒编辑重载 | `python3 -I scripts/test_posix_viewer.py` |
| 公共入口端到端 | 通过 | `python3 -I scripts/test_posix_e2e.py` |
| restart 端到端 | 通过（旧进程退出→session 保存→新进程→companion 保留→正常关闭） | `python3 -I scripts/test_posix_restart_e2e.py` |
| GUI driver（真实窗口） | navigation、class_boxes、search、source_window、source_jump、review_fixes、mail、metrics、inspector、detail_scroll、fonts 全部通过 | `python3 -I scripts/run_love_driver.py <name>` |

端到端覆盖：公共 `./uml` 启动 → companion ownership 写入 → worker `:ready` →
场景发布 → FIFO 出队 → 正常关闭 → ownership 清空 → Herdr 会话删除。
证据见 `e2e-result.json`、`viewer.log`。渲染像素见 `viewer-window.png`、
`viewer-scene.png`。

## 人工体验（WSLg 实机观察）

已确认：中文界面与中文类名/提案名渲染、类框与依赖边、质量红色标记、
主题配色、inspector 布局、图与快照变化触发重载。

**待用户确认**：中文输入法组合输入（需宿主机 IME 会话，见
`wsl-ime-platform-investigation.md`）、剪贴板往返、窗口缩放的实际手感、
Herdr 终端 attach 的可读性。

## 本轮修复

- **CJK 字体供给**：#82 的 Sarasa 供给原先只在已退役的 PowerShell 安装器中
  存在。POSIX 安装器现在补上同一步骤：下载钉住的 TTF 7z、校验归档与
  `SarasaMonoSC-Regular.ttf` 的 SHA256（沿用 #82 的 pin），用
  `.uml-viewer/installer/.venv` 里的 `py7zr` 解出该字重；任一失败即非零退出。

## 未执行/阻塞

- macOS Apple Silicon 实机（#115–#119）：本机无该硬件，全部未执行。
- 真实 Claude companion 会话：自动验收使用无害占位进程验证生命周期，
  允许展示/拒绝展示的真实工具门控证据仍需真实会话一次。
- Windows 本地环境最终清理（#120）：需用户逐项确认清单后执行。
