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
| 双解释器纯逻辑 | 695 / 695 通过（Lua 5.4、LuaJIT、`lua` 5.1） | `lua5.4|luajit scripts/run_busted.lua` |
| 全量 POSIX 检查 | 4 / 4 通过 | `python3 -I scripts/check.py` |
| POSIX 原生进程行为 | 12 / 12 通过 | `luajit scripts/run_posix_native_busted.lua` |
| 公共入口（安装/IR/质量） | 29 / 29 通过 | `python3 -I scripts/test_posix_entries.py` |
| restart 拒绝规则 | 5 / 5 通过 | `python3 -I scripts/test_posix_restart_entries.py` |
| viewer 渲染与重载 | 通过，含亚秒编辑重载 | `python3 -I scripts/test_posix_viewer.py` |
| 公共入口端到端 | 通过 | `python3 -I scripts/test_posix_e2e.py` |
| restart 端到端 | 通过（旧进程退出→session 保存→新进程→companion 保留→正常关闭） | `python3 -I scripts/test_posix_restart_e2e.py` |
| durable recovery 端到端 | 4 个场景 + 旁观项目全部通过（见下） | `python3 -I scripts/test_posix_recovery_e2e.py` |
| GUI driver（真实窗口） | navigation、class_boxes、search、source_window、source_jump、review_fixes、mail、metrics、inspector、detail_scroll、fonts 全部通过 | `python3 -I scripts/run_love_driver.py <name>` |

端到端覆盖：公共 `./uml` 启动 → companion ownership 写入 → worker `:ready` →
场景发布 → FIFO 出队 → 正常关闭 → ownership 清空 → Herdr 会话删除。
证据见 `e2e-result.json`、`viewer.log`。渲染像素见 `viewer-window.png`、
`viewer-scene.png`。

## durable recovery（#113）

公共 `./uml` 在隔离的 XDG runtime 中启动，companion 是一个按指令以指定退出码
退出的占位进程。一个旁观项目全程运行，同一 runtime 中另有无关 Herdr 会话
`bystander` 和一个无关 `sleep` 进程。每个场景结束后都核对这三者存活且未被
改动。证据位于 `recovery/<场景>/`（`starts.log` 记录每个子进程的实际启动
时刻，另有 `recovery.edn`、recovery 日志和 `viewer.log`），汇总见
`recovery/result.json`。

| 场景 | 结果 |
|---|---|
| cancel | 两次异常退出后，journal 记录的重试时刻与 runner 记录的退出时刻分别相差 1.02 s、2.07 s（预算 1/2 s），子进程分别在 1.92 s、2.63 s 后重启；恢复后邮件按 FIFO 出队且无拒绝；第三次退出进入 4 s 退避后关闭 viewer，journal 为 `:cancelled`，controller 退出，超过预算后也没有复活 |
| exhausted | 三次恢复的调度时间为 1.05、2.09、4.10 s；第四次退出后 journal 为 `:failed "Recovery exhausted after three retries"`，没有第五个子进程 |
| normal | exit 0 后 journal 为 `:normal`，不重启 |
| unknown | runner 收到 SIGHUP 时还没记录退出，之后子进程消失；journal 为 `:failed "Runner child disappeared without a matching exit; recovery unsupported"`，不重启 |

本 e2e 未覆盖的部分：
- 关闭用 SIGTERM 模拟，它与窗口管理器的退出请求走同一条 `love.quit`
  路径；真实 WSLg 窗口关闭按钮未自动化。
- 「关闭恰好发生在 relaunch 进行中」的竞态、ownership 的 unknown/legacy
  状态以及 PID 复用，只由单元测试和 native harness 覆盖。
- 旁观进程中不包含另一个真实 Claude 会话；真实 Claude 的恢复证据见下文 #114 一节（`real-companion/run4-recovery/`）。
- `:context` 邮件只验证了出队，没有验证 viewer 应用了它（应用逻辑由
  mailbox 单元测试覆盖）。

### 发现：Herdr server bootstrap 慢

第二个项目启动时首次 `workspace create` 超过了普通命令的上限。原因是 server
的 socket 一开始监听，`status` 就返回成功，而首个 `workspace create` 要等
bootstrap 完成。bootstrap 期间会遍历 PATH，WSL 在 PATH 中追加的 64 个
`/mnt/c` 项都要经 9P 访问。实测 bootstrap 在空闲时为 5.7 s，有另一个项目
并存时为 10.2 s；去掉这些 PATH 项后为 0.17 s。现在 `workspace create` 单独
使用 60 s 的上限，其余命令的上限不变（`love/uml/adapters/herdr.lua`，见
`docs/herdr-backend-contract.md`）。

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

## 真实 Claude companion 全流程（#114）

2026-10-21，证据位于 `real-companion/`。companion 是真实 Claude Code
2.1.306（`~/.local/bin/claude`，经本机 Magpie 网关，transcript 记录的 model 为
`claude-opus-5-5`），没有用 `CLAUDE_BIN`、fixture 或占位 worker。三个质量工具
都是公共安装入口装入的真实副本。

### 环境与来源

| 项 | 值 |
|---|---|
| 隔离项目 | `/tmp/uml-114/库存 项目`（中文+空格），Lua 3 模块 + busted spec，由 `harness/make_project.py` 生成 |
| 安装 | `scripts/get-uml-viewer --install-only`，`UML_VIEWER_TOOL_ROOT` 指向本 worktree（开发符号链接）；exit 0，见 `install.json` |
| uml-viewer | 本 worktree，安装时为 `6c10289`，修复后运行时为 `e21ae14` |
| crapper / mutator / dryer | `main` 的 `34977fc` / `bcbdc42` / `0941c4b`（GitHub 浅克隆） |
| LOVE / Herdr / Python / Lua | `/usr/bin/love` 11.5 / 0.9.5 protocol 22 / 3.12.3 / lua5.4 + 用户 LuaRocks（busted 2.3.0、luacov 0.17.0、cluacov 1.0.0、luacov-reporter-lcov 0.2） |

邮件触发方式：`harness/driver.lua` 经 `UML_VIEWER_TEST_DRIVER` 加载进真实 viewer，
收到 harness 放置的触发文件时，调用右键菜单使用的 `hooks.mail.menu_action`。
outbox 写入与 Herdr wake 都走产品自己的路径。driver 另外每 30 帧记录 viewer 实际
状态（`driver.log` 中的 `viewer state`），状态变化时截图（`shot-*.png`）。
`UML_COMPANION_NO_ATTACH=1`：不经 interop 拉起 Windows Terminal，attach 留给人工清单。

### 自动断言：步骤结果

| 步骤 | 结果 | 证据 |
|---|---|---|
| 安装 | 通过 | `install.json` |
| 手动质量基线：`./uml crap` / `dry` / `mutate app/pricing.lua` | 都是 exit 0，version-1 记录有效 | `manual-*.log` |
| fresh `./uml` + companion ready | 通过；worker `:ready`。**首次在新目录**，Claude 停在 workspace 信任对话框，默认选项是 “No, exit”；按用户操作选了 Yes（Herdr send-keys） | `run1-trust/pane-00-trust-dialog.txt` |
| 启动提示 | Claude 保留 policy 与 `出库提案`，`./uml ir` exit 0 后写 `:display`，viewer 消费 | `run1-trust/transcript.txt`、`run2-wake-bug/driver.log` |
| 右键邮件 wake | **修复前全部失败**（缺陷 1） | `run2-wake-bug/driver.log`、`pane-00-launch.txt` |
| `:quit-for-restart` → `./uml --restart` | **修复前崩溃**（缺陷 2）；修复后恢复保存的图、mail cursor，companion session 与 shell PID 不变，controller PID 不变 | `run3-gates/viewer-restart-crash.log`、`restart2.log` |
| 允许展示 A：`:refresh-crap` ×2（FIFO id 1、2） | crap exit 0 → 记录有效 → IR exit 0 → `:display` id 4、5；viewer mail-seen 3→5，metrics-stamp 更新 | `run3-gates/pane-A-allowed.txt`、`driver.log` 状态 3–5 |
| 拒绝展示 B：spec 期望值改成错误值后 `:refresh-crap` | 真实 busted 失败 → crapper exit 1，`validResult false` → **没有 IR、没有 `:display`**；Claude 报告失败断言 `inventory_spec.lua:7`，不改源码 | `pane-B-refused.txt`；`driver.log` 在 09:57:38–09:58:33 没有状态变化 |
| 拒绝展示 D：`uml-quality.json` 写成非法 JSON 后 `:refresh-mutate` | dispatch exit 1，**没有发布记录** → 没有 IR/display；Claude 报告 JSON 解析错误位置 | `pane-D-C.txt` |
| 后续 FIFO C：紧随 D 的 `:omit pricing` | 加入 policy `:omit` → IR exit 0 → `:display` id 6；viewer 显示 pricing 已隐去 | `pane-D-C.txt`、`shot-6.png` |
| `:context` 提案 + 允许的 `:refresh-mutate`（exit 3） | Claude 记下讨论 context；mutator exit 3 按工具契约有效 → IR → `:display` id 7；存活变异体保留为发现 | `pane-E-context-mutate.txt` |
| recovery（SIGTERM 真实 Claude 子进程） | **修复前**：重试用 `--session-id`，Claude 每次报 “Session ID … already in use” 后 exit 1，三次后 `:failed`（缺陷 3） | `run3-gates/recovery-before-fix/`、`pane-F-recovery-failed.txt` |
| recovery（修复后，run 4） | exit 143 → 1.04 s 后第 1 次重试；子进程为 `claude --resume <同一 session>`；恢复提示排空上一轮遗留的邮件 id 8（crap → 门控 → IR → `:display`）；之后新的右键 wake 也正常（mail-seen 10） | `run4-recovery/` |
| 正常关闭（SIGTERM viewer，四轮都做） | ownership `{}`、Herdr session 删除、journal `:cancelled`（run 3 为既有的 `:failed`）、controller 与 pane shell 退出 | 本节与各 run 记录 |
| 隔离 | 用户主工作区 `uml-viewer-uml-viewer-7e504677-ac364c68`、`default` Herdr session 与运行本 agent 的 Claude 都存活；没有用 SIGKILL。基线中另一个 Claude PID 934176 在期间自行退出：没有向它发信号，主工作区 companion 在本次开始前（16:50）已 `:cancelled` | — |

“陈旧记录”没有单独构造：dispatch 在每次调用开始时先删旧记录，companion 也先删，
所以 D 实测的“缺失”就是陈旧记录在该契约下唯一可能的样子。

### 本轮修复的产品缺陷（TDD，红→绿）

1. **右键邮件无法唤醒 companion**（`5448cc4`）：worker 发出的提示是
   `Mailbox mail <id>`，token 是 `mail-<id>`；Herdr wake 契约要求提示原文包含
   token，因此每次 wake 都在发送前被拒绝。既有 e2e 只覆盖 to-viewer FIFO，
   所以没有暴露。
2. **`./uml --restart` 在没有 `examples/` 的项目中崩溃**（`714f9fc`）：默认图查找
   `lfs.dir('./examples')` 先于读取保存的路径执行并抛错。查找移到
   `adapters/session.default_diagram`，并能容忍目录不存在。
3. **真实 Claude 无法 recovery**（`e21ae14`）：lifecycle 只把 `--resume` 恢复命令
   与 Claude session id 写进 provisional 记录，构造 backend 时没有带上这个 identity，
   committed `companion.edn` 因此丢失这两项。占位进程接受任意 argv，所以 #113 的
   e2e 无法发现。

质量流程（串行）：`./uml crap` exit 0；逐文件 differential mutate：
companion_worker 87/17/11（exit 3）、companion_lifecycle 46/21/17（exit 3）、
session 11/4/0（exit 3）、viewer 0/0/187（exit 0，GUI 适配器无覆盖，属覆盖缺口）；
`./uml ir` exit 0（`105b509`）。回归：双解释器 699/699，POSIX 原生 14/14，
`test_posix_e2e.py`、`test_posix_restart_e2e.py` 通过。

### 观察（未改动）

- 中文项目名在 Herdr session 名中全部变成 `-`（`uml-viewer---------------c594b08c-…`），能用但难以辨认。
- 新目录首次启动时，信任对话框只有 attach 后才看得见；未 attach 时 worker 仍然发出
  `:ready`，Claude 却在等待输入。
- 用时超过 5 s 的邮件会让状态行显示 “wake consumption was not acknowledged”，即使
  Claude 随后正常处理（B 中 8 s 后出队）。
- 测试证据 `scene-N.edn` 只在文件重载时写出，mail `:display` 不会写；本次改用 driver 的状态记录观察。
- `./uml mutate --help` 报“没有业务 mutation 目标”，而不是输出用法。
- recovery 命令的 argv[0] 是裸 `claude`，launch 命令是解析后的绝对路径；实测可以运行。

## 未执行/阻塞

- 真实 WSLg 窗口关闭按钮仍未自动化（关闭用 SIGTERM，与 `love.quit` 同一路径）。
- Windows Terminal attach 没有在本轮自动运行（避免跨边界调用 Windows 二进制），列入人工清单。
- Windows 本地环境最终清理（#120）：需用户逐项确认清单后执行。

## 人工体验清单（待用户在 WSLg 确认）

先在任意已安装的项目执行 `./uml`（会打开 viewer 窗口，并通过 Windows Terminal
attach companion）。

1. **窗口**：拖动边缘改变大小，最大化后再还原；viewer 应随之重排，不黑屏、不闪退。点关闭按钮后，Herdr 会话应消失（`herdr session list`）。
2. **导航**：双击命名空间进入，Esc 返回；滚轮平移、Shift+滚轮横向平移、Ctrl+滚轮缩放、空白处拖拽平移、Ctrl+0 适配全图。
3. **提案**：在 inspector 点「新建提案」并改名为中文，点「出库提案」等已有提案，再按 P 返回命名空间树；policy 中的 `:proposals` 应保留。
4. **inspector**：拖动 inspector 左边缘改变宽度；单击类查看卡片；在搜索框输入类名或命名空间，跳转应正确。
5. **中文输入/组合**：在搜索框和提案改名框用 Windows 输入法输入拼音，候选窗应跟随光标，上屏的中文正确，退格逐字删除。
6. **剪贴板**：在 Windows 记事本复制一段中文，粘贴到 viewer 搜索框；再从 viewer 输入框复制，回到 Windows 粘贴，两个方向都要试。
7. **缩放**：Ctrl+滚轮放大缩小，确认文字清晰、类框不错位；有多显示器时把窗口拖到不同 DPI 的屏幕上看。
8. **Herdr attach**：确认 Windows Terminal 新标签页自动打开并显示 Claude；中文回复可读；直接输入一句话能看到回显；`Ctrl+B q` detach 后，`herdr session list` 中会话仍在。新目录首次启动时，在这里回答信任对话框（选 Yes）。
9. **右键质量刷新**：右键一个类，选择刷新 CRAP；Claude 处理后图上的 C 标记更新，状态行没有 “wake failed”。
