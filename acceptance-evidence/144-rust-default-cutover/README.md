# #144 验收记录：默认 rust、删除 Lua 实现、自举 `:lang :rust`

日期：2026-10-22。环境：WSL2 + Ubuntu 24.04 + WSLg（NVIDIA GeForce GTX 1080 Ti），
Rust 1.99.1（`rust-toolchain.toml`），egui/eframe 0.37 + wgpu GL，Herdr 0.9.5 /
protocol 22，Python 3.12，cargo-llvm-cov 0.9.1。所有运行由 agent 执行；没有推算值。
除非另有说明，命令都在本仓库根目录、默认 runtime（无 `--runtime` / `UML_RUNTIME`）下执行。

## 1. 验收标准核对

| 标准 | 结果 | 证据 |
|---|---|---|
| 安装器不再需要 LOVE、LuaJIT、Lua 5.4、lfs、luacov、cimgui | 通过 | 见 §2；`test_posix_entries.py` 46/46，其中安装器用例断言没有 `love_command`/`lua_command`/`cimgui`/`CIMGUI_URL`/`LUA_PROBE`，`install.json` 只有 `rust`/`herdr`/三个工具仓库 |
| Seam A、Seam B 全部测试在默认 runtime 下通过 | 通过（有重检） | 见 §3、§4；首轮 4 个失败（3 个 driver 期望与 recovery 超时）修正后各重检一次通过 |
| 本仓库 IR 由 Rust 源码生成，质量快照来自 Rust 分析 | 通过 | 见 §5；77 个 Rust 类，76 个与 crapper Rust 命名空间连接 |
| 仓库内不再有 Lua 运行时代码 | 通过 | 见 §6；`love/`、busted 规格、LOVE/cimgui 供给与 Lua 工具脚本已删除 |

## 2. 入口与安装器

`--runtime rust` 是默认值；`--runtime love` 与 `--runtime jvm` 在 dispatch 前以
`--runtime 只接受 rust（默认）；love 与 JVM 已移除。` 拒绝并退出 1：

```text
$ ./uml --runtime love ir
uml: --runtime 只接受 rust（默认）；love 与 JVM 已移除。
love-exit=1
```

`./uml ir`、`./uml crap`、`./uml mutate`、`./uml dry` 都不带 `--runtime` 也走 Rust。
安装器预检只有 `git`、Herdr >= 0.9.3 / protocol 22、`cargo`（版本由
`rust-toolchain.toml` 固定）与 WSL 平台；随后 `cargo build --release --locked`。
`UML_VIEWER_TOOL_ROOT` 的开发仓库检查改为 `Cargo.toml`（不再是 `love/main.lua`）。
`install.json` 不再写 `love` 键。

## 3. Seam A（进程边界）

| 套件 | 结果 | 备注 |
|---|---|---|
| `python3 -I scripts/test_posix_entries.py` | PASS：46 tests OK，189.029 s，exit 0 | 包含安装器幂等、真实 Rust IR（四个扫描器 + 本仓）、质量分发、默认 runtime 与 LOVE/JVM 拒绝、launcher/preflight |
| `python3 -I scripts/test_posix_restart_entries.py` | PASS：6 tests OK，0.790 s，exit 0 | 新增 `test_restart_refuses_a_removed_love_record` |
| `python3 -I scripts/test_posix_viewer.py --evidence /tmp/uml144-viewer` | PASS，exit 0 | 真实窗口、热重载（`scene-2.edn`）、`render.png` 340210 B、正常退出 |
| `python3 -I scripts/test_posix_e2e.py` | PASS，exit 0 | launch/companion/FIFO/close；ownership 清空、Herdr session 删除 |
| `python3 -I scripts/test_posix_restart_e2e.py` | PASS，exit 0 | companion 保留、新 viewer 退出、ownership 清空 |
| `python3 -I scripts/test_posix_recovery_e2e.py` | 首次 FAIL，重检 PASS | 首次在 exhausted 场景等待 `recovery 2` 超时（journal 已 `:running`/attempts 2，但 `starts.log` 只有两次启动）；单独重检一次通过，日志见 `logs/recovery_e2e.txt` 与 `logs/recovery_e2e-rerun.txt` |

`test_posix_entries.py` 的 IR 对照改由 `scripts/compare_ir.py` 做结构化 EDN 比较，
不再调用 `lua`。

## 4. Seam B（viewer driver）

`python3 -I scripts/run_viewer_driver.py <driver> --evidence <dir>`。16 个 driver 的
首轮全量运行一次；三个失败项在修正 driver 期望后各重检一次（见下表），其余未重跑。
证据文件（截图、scene EDN、session/邮件）当时写在 `/tmp/uml144-driver-<name>/` 与
`/tmp/uml144-rerun-<name>/`；现已原样拷入本目录的 `evidence/drivers/` 与
`evidence/rerun/`（`drivers/*.json` 的 `evidence` 字段保留当时的 /tmp 路径，作为
来源记录）。16 个 driver 与 3 次重检的文件数逐一核对一致，没有只留在 /tmp、
无法再查看的声明；`evidence/viewer/` 是 Seam A viewer 运行的证据。

| driver | 首次结果 | 备注 |
|---|---|---|
| `class_boxes` | FAIL → 重检 PASS | 第 4 帧自动截图与脚本结束时的 viewport close 竞争，未写出 `render.png`；driver 改为显式 `:screenshot` 步骤后通过 |
| `detail_scroll` | PASS | 卡片滚动 48pt 步进、内容夹取 |
| `dpi_window` | PASS | DPI 角标与窗口尺寸 |
| `font_parity` | FAIL → 重检 PASS | baseline 仍用 LOVE IR 的 `uml`/`uml.application.detail`；Rust IR 已无这些类，改为 `ir → ir.graph` 后通过（chinese 场景不变） |
| `fonts` | PASS | Sarasa override/fallback/覆盖 |
| `inspector` | FAIL → 重检 PASS | Rust mail-only companion 的唤醒成功，状态为 “Regen requested.”（旧断言要求 “Regen queued”）；driver 对齐状态文本，`:op :regen` 邮件断言保留 |
| `lod_shots` | PASS | 三档 LOD 截图与 renderer |
| `mail` | PASS | 队列消费与去重 |
| `metrics` | PASS | 两版场景的质量叠加 |
| `navigation` | PASS | fit/下钻/锚点缩放/拖拽/Esc/命中/滚轮/Declutter |
| `review_fixes` | PASS | 独立源码窗口与剪贴板 |
| `search` | PASS | 搜索框焦点与画布输入隔离 |
| `session` | PASS | 保存/恢复/legacy/LOVE 写下的 session 数据 |
| `source_jump` | PASS | module/member 跳转与高亮 |
| `source_window` | PASS | 源码窗口滚动 |
| `companion_worker` | PASS | 真实 Herdr + 占位 agent，等待 UI |

三个首次失败的 driver 都只改了 driver 期望（`scripts/drivers/*.edn`），没有改
viewer 运行时；各重检一次通过，`result.json` 的 `ok:true` 在
`drivers/<name>.json`。重检覆盖了同名 `.txt` 日志，首次失败原因保留在本表与
`drivers/summary.txt` 的 `exit=1` 行。

两处 driver 期望变化的来源（都是 #144 的合法后果，不是放宽断言）：

- `inspector` 的状态文案：driver 不带 `--backend` 运行，Rust 的 mail-only companion
  仍记录唤醒尝试并成功唤醒，`app.rs` 在 `wake` 成功后显示 “Regen requested.”；
  只有未连接 companion 时才显示 “Regen queued; Claude session not attached.”。
  旧断言来自 #141：当时唤醒尚无 Rust companion（属 #142），只写邮件，所以断言的是
  未连接文案；#144 的 driver 运行已有 mail-only companion，唤醒成功。权威断言仍是
  `to-agent.edn` 中的 `:op :regen` 邮件，状态文案只描述唤醒结果。
- `font_parity` 的 baseline 类 id：本仓库 IR 现在由 Rust 源码生成（policy
  `:lang :rust`、`:sources crates/<crate>/src`），命名空间为 `uml.ir`，class id
  为 `ir.graph`；LOVE 时代的 `uml` / `uml.application.detail` 在 Rust IR 中不再
  存在。driver 因此下钻 `:ir` 并选中 `ir.graph`；断言本身（文字落在槽内、
  质量诊断）没有变化。

## 5. 质量快照与 IR

命令按 AGENTS.md 的顺序执行（质量调用串行）：

1. `./uml crap`（`standardTools.sourceRoots: ["crates"]`，Rust coverage 由
   `cargo llvm-cov` 产出）：exit 0，`.uml-viewer/quality-result.json` 为
   `{"version":1,"operation":"crap","projectRoot":"<本 worktree>","exitCode":0,
   "validResult":true,"crapScope":"whole-tree","configured":false,"tool":"crapper"}`。
   `.metrics/crap.edn` 有 1223 条函数记录、134 个 Rust 命名空间。最后一次源码
   改动（`crates/companion/src/domain.rs` 的提示文本）后复跑一次，记录与计数相同。
2. `./uml mutate`（差分，每个文件一次；每次先删除旧记录并串行等待真实退出）：
   六个文件的记录都是 `version:1`、`operation:"mutate"`、`projectRoot` 为本
   worktree、`validResult:true`、`exitCode:3`（有存活/未覆盖变异体是有效结果）。

   | 文件 | 变异体 | killed | survived | uncovered | score | 退出 |
   |---|---|---|---|---|---|---|
   | `crates/ir/src/detection.rs` | 7 | 6 | 1 | 0 | 85.7% | 3 |
   | `crates/ir/src/lib.rs` | 22 | 17 | 4 | 1 | 81.0% | 3 |
   | `crates/scene/src/document.rs` | 75 | 38 | 16 | 21 | 70.4% | 3 |
   | `crates/companion/src/lifecycle.rs` | 20 | 3 | 7 | 10 | 30.0% | 3 |
   | `crates/companion/src/role.rs` | 16 | 1 | 6 | 9 | 14.3% | 3 |
   | `crates/companion/src/domain.rs` | 75 | 63 | 9 | 3 | 87.5% | 3 |

   存活与未覆盖按“findings”保留，不重跑追分（AGENTS.md）。`lib.rs` 的第一次
   运行有一个 `--policy`/`--output` 位置变异体把四个 `*-fixture/policy.edn`
   改写成了生成的 IR；fixture 已从 HEAD 恢复，并把四个 fixture 目录临时设为
   只读后单独重跑该文件，结果与第一次相同（17/4/1），因此上表采用重跑记录。
   其余未覆盖集中在 `MemoryBoundary` 的 `write`、`selected_target` 和
   companion 的 `open_window`/`shutdown` 等无测试路径；`document.rs` 的
   `selected_target` 有 9 个未覆盖位点。
3. `./uml dry`：exit 0，记录 `validResult: true`，`.metrics/dry.edn` 来自 Rust 源码。
4. `./uml ir`：exit 0，写出 `examples/uml-viewer.edn`（文件头
   `;; Generated by uml ir ...`）。

策略改为 `:lang :rust`，每个 crate 一个 `:sources` 根
（`crates/<crate>/src`，ns 前缀 `uml.<crate>`），`:proposals` 原样保留
（`:p-1791033119418`），`:levels`/`:order`/`:foreign` 按 crate 树更新。
IR 有 77 个类、314 条边；质量连接按 class id 匹配 `crate::module`
（77 个类中 76 个匹配；`crates/uml-viewer/src/main.rs` 生成
`main.main`/`uml.main.main`，而 crapper 命名该二进制 crate 为 `uml_viewer`，
所以它保持红色——这是已知的覆盖缺口，不隐藏）。

## 6. 保留的 Lua 引用

- `lua-fixture/`：Lua 扫描器的 fixture 项目（`src/calc/*.lua`、fixture 自带的
  `spec/`、`.busted` 与 `.metrics`），用于扫描、质量连接与 IR golden；不是
  本仓库的运行时代码或测试套件。
- `acceptance-evidence/106-wsl/real-companion/harness/driver.lua` 与
  `acceptance-evidence/142-mail-session-restart/love-session/love-probe.lua`：
  历史验收证据的原字节，不改写。
- `acceptance-evidence/142-.../love-session/session.edn` 与
  `scripts/drivers/session_love.edn`：真实 LOVE viewer 写下的 session 数据，
  作为 Rust viewer 向后兼容恢复的 fixture；属于数据契约，不是运行时。
- Rust 源码里的 `//! Port of love/uml/...` 注释：移植来源说明，保留。
- `bench/results/love-*` 与 `bench/README.md` 的历史 LOVE 测量：保留为选型证据。

其余 `.lua` 文件（`love/`、`love/spec`、busted 配置、Lua 工具脚本、LOVE
驱动、benchmark 生成器/驱动）已删除。

## 7. 需人工验证（未观察）

- 在本机没有 LOVE/Lua 的全新用户目录运行一次 `get-uml-viewer`（真实网络下载
  Sarasa 与三个质量工具，真实 `cargo build`），确认没有隐式 Lua 依赖。
- 物理鼠标右键菜单、真实窗口关闭按钮、系统剪贴板和 WSLg 输入法组字
  （driver 注入 egui 事件，不是物理输入）。
- 真实大项目下的 `./uml crap`/mutation 时长与 Rust 覆盖率行为。
