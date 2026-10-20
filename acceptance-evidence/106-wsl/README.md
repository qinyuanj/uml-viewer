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
| 双解释器纯逻辑 | 681 / 681 通过（Lua 5.4、LuaJIT；`lua` 5.1 为 682 / 682） | `lua5.4|luajit scripts/run_busted.lua` |
| 全量 POSIX 检查 | 4 / 4 通过 | `python3 -I scripts/check.py` |
| POSIX 原生进程行为 | 12 / 12 通过 | `luajit scripts/run_posix_native_busted.lua` |
| 公共入口（安装/IR/质量） | 29 / 29 通过 | `python3 -I scripts/test_posix_entries.py` |
| restart 拒绝规则 | 5 / 5 通过 | `python3 -I scripts/test_posix_restart_entries.py` |
| viewer 渲染与重载 | 通过，含亚秒编辑重载 | `python3 -I scripts/test_posix_viewer.py` |
| 公共入口端到端 | 通过 | `python3 -I scripts/test_posix_e2e.py` |
| restart 端到端 | 通过（旧进程退出→session 保存→新进程→companion 保留→正常关闭） | `python3 -I scripts/test_posix_restart_e2e.py` |
| GUI driver（真实窗口） | navigation、class_boxes、search、source_window、source_jump、review_fixes、mail、metrics、inspector、detail_scroll、fonts 全部通过 | `python3 -I scripts/run_love_driver.py <name>` |

端到端覆盖：公共 `./uml` 启动 → companion ownership 写入 → worker `:ready` →
场景发布 → FIFO 出队 → 正常关闭 → ownership 清空 → Herdr 会话删除。
证据见 `e2e-result.json`、`viewer.log`。渲染像素见 `viewer-window.png`、
`viewer-scene.png`。

## 质量工具（公共 `./uml` 分派，串行）

每次调用前删除 `.uml-viewer/quality-result.json`，调用后核对新写出的
version-1 记录（operation、项目根、实际退出码一致，`validResult: true`）。

- `./uml crap`：exit 0，`.metrics/crap.edn` 已更新。
- `./uml dry`：exit 0，报告 5 处重复结构（终端读取，无叠加层）。
- `./uml mutate`：按变更源文件逐个差分运行。exit 3（存在存活变异体）的
  文件保留快照；未覆盖变异体视为覆盖缺口，不重跑。例：herdr
  139/71/5（66.2%）、companion_worker 73/19/16（79.3%）、mailbox 13/6/0、
  recovery 56/31/71、domain/herdr 71/29/3、domain/companion 104/17/0。
- 执行失败而非结论：mutator worker 池指纹在运行中被改动时报 exit 2、无快照。
  一次是运行期间改动了工作区（静止后重跑通过）；另一次是 herdr_spec 的
  `cwd='C:/project'` 在 POSIX 上是相对路径，绕过注入写入器的变异体把
  `companion.edn` 写进了 worker（已改为绝对路径夹具，d1fec15）。
- 工具自身 pytest：crapper 251 通过 / 11 跳过 / 1 失败（需 Babashka `bb`，
  本机未安装，属环境缺口）；mutator 209 / 40 跳过；dryer 84 通过。

## 性能（对照 #87 基线，同一台 Ryzen 5 5500）

驱动 `scripts/benchmark_viewer.lua`，与 #87 同一相位划分；WSLg 下直接以
`love love viewer <diagram>` 运行（例子 16 s，合成 500 类 90 s），Sarasa 字体。
原始输出见 `performance/`。

| 场景 / 相位 | #87 Windows（fps / mean ms） | #106 WSLg（fps / mean ms） |
|---|---|---|
| 例子 idle | 59.4 / 16.8（vsync 上限） | 122.3 / 8.2 |
| 例子 interaction | 43.8 / 22.8 | 45.0 / 22.2 |
| 合成 500 idle | 1.58 / 632 | 9.77 / 102 |
| 合成 500 interaction | 1.61 / 619 | 2.02 / 495（p95 3.67 s，单帧尖峰） |

无回退；合成场景交互 p95 出现秒级单帧尖峰，记录在案，不设阈值。

字体：e2e 项目未经安装器，日志里的 Noto 回退是预期行为；Sarasa 供给由
`fonts` driver 的 provisioned / override / fallback 三个场景覆盖并通过。

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
