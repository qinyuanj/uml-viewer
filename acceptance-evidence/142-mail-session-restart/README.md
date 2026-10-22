# #142 验收记录：mail、session 与 `--restart`

环境：WSL2 + WSLg，`renderer: D3D12 (NVIDIA GeForce GTX 1080 Ti)`，wgpu GL 后端。
GUI driver 由 `python3 -I scripts/run_viewer_driver.py <driver>` 在 Rust runtime 下重放
（`raw_input_hook` 注入 egui 事件），退出码与证据文件是断言来源。

## 1. driver 改写为 EDN 脚本并通过

| driver | 脚本 | 结果 | Lua 断言 → EDN 断言 |
|---|---|---|---|
| mail | `mail.edn` | PASS | 两条 `to-viewer` 命令（`:context` 提案 + `:display second.edn`）按 FIFO 消费：`mail-seen` 2、不再等待、标题 `Second diagram`、讨论 context 为提案；对选中类依次点右键菜单四项，`to-agent.edn` 与 LOVE 的精确契约逐字（EDN）相等：`{:next-id 5 :queue [{:id 1 :op :refresh-crap :target {:id :target :kind :class :ns "app.target"}} …]}`；随后 `:context :real` 与 `:quit-for-restart` |
| companion_worker | `companion_worker.edn` | PASS | 隔离 XDG + 真实 Herdr + 占位 Claude 进程（`CLAUDE_BIN`、`UML_COMPANION_NO_ATTACH=1`）：`:display` 前是等待空画布（`waiting.png`），状态行 `Companion: ready` 后写 `worker-ready.txt`；写入 `:context :real` 后讨论上下文生效且队列出队；不出现 `ownership retained` |
| review_fixes | `review_fixes.edn` | PASS | 两个源码窗口各自保留选区（`FIRST` / `SECOND`）；Ctrl+C 复制聚焦窗口的 `SECOND`；再在搜索框复制另一段文本后，两个窗口的选区与源码复制结果都未被覆盖。Lua 版断言 cimgui 剪贴板回调；egui 记录的 `:copied` 是 viewer 交给平台的文本，系统剪贴板本身见「需人工验证」 |
| session | `session.edn` + `session_restore.edn` + `session_legacy.edn` + `session_love.edn` | PASS | 见下节 |

## 2. session 保存/恢复（含 LOVE 写的 session.edn）

同一个隔离项目依次启动四次（`run_viewer_driver.py session`）：

1. **save**：拖检查器边缘到 340、真实选中并双击打开类卡片、缩放与移动视图、窗口改
   为 1100x700，再发 `:quit-for-restart`。viewer 自己把 `session.edn` 写到
   `.uml-viewer/`（schema 与 LOVE 相同），退出码 0。
2. **restart**（`--restart`）：窗口 1100x700、检查器 340、zoom 1.7、类卡片、mail 游标
   全部恢复，且**不重新 fit**；退出前 dump 的快照与第 1 步的快照逐字段（EDN）相等
   （仅 `:proposal` 派生字段因校验器补写而忽略）。恢复后写入 id 2 的 `:context` 仍被
   消费到 `mail-seen` 2——游标没有跳过邮件。
3. **legacy**：没有窗口字段的旧 `session.edn` 以默认 1500x920 / 检查器 280 打开，
   视图（zoom 1.25、cam-x 7）与游标 4 照旧保留。
4. **love**：把真实 LOVE 11.5 viewer 在 `:quit-for-restart` 时写下的
   `session.edn`（`love-session/session.edn`，只把绝对 `:path` 换成本项目）放进去，
   Rust viewer 以 `--restart` 恢复：窗口 1100x700、检查器 400、zoom 1.7、
   cam (33,21)、选中项与类卡片 `:a`、讨论 context（提案 `:plan`）、mail 游标 2；
   截图 `love-session-restored.png`，GUI 记录 `love-session-gui.edn`。

LOVE 侧证据：`love-session/` 含 `session.edn`（真实字节）、生成它的 `love-probe.lua`
（选择与卡片来自真实指针点击，zoom/cam 由 probe 通过 LOVE 的状态字段设置，与用户
缩放/平移等价）与那次运行的截图。`imgui.ini` 不再需要（egui 不写该文件）。

## 3. restart 相关入口与 e2e（`--runtime rust`）

`UML_RUNTIME=rust` 让现有四个测试套件走 Rust runtime，断言未改动：

| 套件 | 结果 | 时间与备注 |
|---|---|---|
| `test_posix_restart_entries.py` | PASS（5 tests OK） | 2026-10-22 20:54，合并后首次运行 |
| `test_posix_restart_e2e.py` | PASS | 21:03，在 runner 重提交修复后运行一次 |
| `test_posix_e2e.py` | PASS（launch / companion / scenes / FIFO / exit / cleanup） | 20:58 |
| `test_posix_recovery_e2e.py` | PASS（neighbour、cancel、unknown 三个场景） | 21:05，在 runner 修复与 Mesa 噪声过滤后运行一次 |

首次运行曾有两处失败，原因与修复：

- `test_posix_restart_e2e` / `test_posix_recovery_e2e` 的 cancel 场景等待 companion ready 超时。Herdr 0.9.5 会在 pane 已输出后仍静默丢弃第一次 `pane run`（返回 0），runner 因此没有写状态。修复：提交后 10 s 内未见 runner 状态即重提交，最多 3 次；runner 有 generation lock，重复提交只会干净退出（`crates/companion/src/herdr.rs`）。
- cancel 场景的日志检查把 Mesa 的 `libEGL warning … failed …` 当成“邮件被拒”。修复：只忽略以 `libEGL warning` / `MESA-LOADER` 开头的行，其余断言不变。

断言口径未放宽：`restart_e2e` 仍在断言旧 viewer 退出、session 保存、companion 保留、新进程替换、最终清理；`recovery_e2e` 仍断言 1 s 预算内恢复、未知退出不重试、旁观会话不受影响。

## 4. 需人工验证（未观察）

- 真实鼠标右键打开元素菜单并点选四项（driver 注入 egui 事件，不是物理鼠标）。
- 真实窗口关闭按钮关闭 viewer 后的 companion 清理（driver/e2e 用测试时长与 SIGTERM）。
- 系统剪贴板：driver 只断言 viewer 交给平台的文本。
- WSLg 输入法组字输入（#140 已列，本 ticket 不涉及输入法）。
- 修改后的 inspector fixture 下 LOVE `--runtime love` 的 inspector driver：已复检通过（`run_viewer_driver.py inspector --runtime love`，exit 0）。

## 5. 已知边界

- companion 在 viewer 异常消失时由 worker 自行检测并清理（`keep=false`），viewer
  正常关闭/收到 SIGTERM 时先发 `stop{keep:false}` 再等待（上限 5 秒）。
- `:regen` 只在存在 companion 时唤醒；无 companion 时仍写入持久队列并显示
  「Regen queued; Claude session not attached.」。
