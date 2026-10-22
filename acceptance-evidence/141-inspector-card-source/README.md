# #141 验收记录：检查器、提案写回、类卡片与源码窗口

环境：WSL2 + WSLg，`renderer: D3D12 (NVIDIA GeForce GTX 1080 Ti)`，wgpu GL 后端。
全部证据由 `python3 -I scripts/run_viewer_driver.py <driver>`（Rust runtime）在
2026-10-22 生成；本目录截图拷自那次运行。GUI 输入由 driver 经 `raw_input_hook`
作为 egui 事件注入（每帧一批），即 egui 自身的指针/键盘/文本路径；**不是**真实
鼠标键盘或系统剪贴板。

## 1. driver 改写为 EDN 脚本并通过

| driver | 脚本 | 结果 | Lua 断言 → EDN 断言 |
|---|---|---|---|
| inspector | `inspector.edn` | PASS | 右键提案 → Rename/Delete 菜单；Rename 显出输入框与「取消」；Ctrl+A + 中文 textinput；Sarasa 覆盖输入文本；侧栏按钮绿底 [42 61 54] 金字 [232 196 72]（截图像素）；P 回命名空间树；另加改名写回、选提案发 `:context`、Regen 写 `:regen` 邮件 |
| detail_scroll | `detail_scroll.edn` | PASS | 空检查器说明；640x700 卡片无横向溢出；48px/格滚动 192、两端夹紧、144；父控件不被滚走；悬停 op8；窄 400 横向 >150、宽 900 内容 >800；New Proposal 加一行；右键菜单 |
| source_jump | `source_jump.edn` | PASS | 真实双击下钻+打开卡片；模块行 → 整文件 84 行、不定位行、真实路径；成员 → 第 82 行高亮、scroll-y>0、可见；拖选得 `function`（不含行号栏、制表符映射回源码）；另加 Ctrl+C 复制选区 |
| source_window | `source_window.edn` | PASS | 按 lua/文件/成员打开落到 82 行、滚动、可见；另加 #140 延后的两个窗口并存、关一个留一个 |

Lua 版 detail_scroll 直接改 state 打开卡片，EDN 版用真实双击。module.txt / member.txt /
source-window.txt 改为 GUI 记录（EDN），Lua 为纯文本；pass marker 只查存在。

## 2. 提案写回保留其他提案与 policy 内容

inspector fixture 现带同级 `diagram.policy.edn`（`:lang :lua :src "love" :prefix "demo"
:omit [:zzz]` + 两个提案 `:plan`/`:keep`）。改名后 driver 断言 policy 含新名
`中文方案输入`、`:keep`/`保留方案`、`:omit [:zzz]`、`:lang :lua`、`:src "love"`，IR 含两个提案。
单测 `crates/viewer/tests/inspector.rs` 另覆盖写回保留键。写回会规范化提案（补
`:omit []`、layer id），与 LOVE 相同。

## 3. 中文提案名改名

inspector.edn：输入框记录 `中文方案输入`，确认后写入 IR 与 policy（截图 inspector-rename.png）。

## 4. #140 延后项

- search：两个源码窗口并存、关闭新的保留旧的（82 行）→ source_window.edn（source-two-windows.png）。
- font_parity：canvas.png 之后双击打开卡片：`:rect {:w 640 :h 700}`、horizontal maximum 0.0、
  row-heights [20.0 18.0]（font-parity-card.png）。原有断言未改。
- dpi_window：侧栏默认 280；从边缘 x=1220 拖到 1160 → 340；Regen 在 x=1176 y=870 h=25（窗口 920 内）。
- 画布临时 Declutter 按钮移入检查器（commands.rs 删除画布 chrome；navigation 的 Declutter 循环改点检查器按钮，仍 PASS）。

## 5. 画布类名 `Parser` vs `:name`

已核对：LOVE `hierarchy.lua` 的 `module_name` → `label(id)` 取 id 末段并首字母大写，
Rust 一致（goldens 钉住）。不是偏差，未改。

## 回归

`cargo test --release --workspace` 全绿；Rust driver 全部 PASS：class_boxes、detail_scroll、
dpi_window、dpi_shots、font_parity、font_parity_baseline、fonts、inspector、lod_shots、metrics、
navigation、search、source_jump、source_window。

## 需人工验证（未观察）

- 真实鼠标右键打开提案菜单、真实拖动侧栏边缘（driver 注入的是 egui 事件）。
- WSLg 输入法组字输入中文提案名（driver 只注入 `Event::Text`，即 IME 提交入口）。
- 源码选区 Ctrl+C 进入**系统剪贴板**（driver 断言的是 egui 收到的复制文本 `:copied`）。
- 卡片窗口真实拖动改尺寸（driver 用 `:resize-card` 强制尺寸）。
- 修改后的 inspector fixture 下 LOVE `--runtime love` 的 inspector driver：本 worktree 无
  cimgui-love 绑定，未运行。
- Regen 唤醒 Claude 会话属 #142；此处只写 `:regen` 邮件并显示「Regen queued; Claude session not attached.」。
