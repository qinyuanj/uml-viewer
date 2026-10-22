# #140 验收记录：导航、搜索、字体 DPI 与中文输入

环境：WSL2 + WSLg，`renderer: D3D12 (NVIDIA GeForce GTX 1080 Ti)`，wgpu GL 后端；
本机 X11 预检未通过，窗口为 Wayland 无装饰（`:decorated false`）。
全部证据由 `python3 -I scripts/run_viewer_driver.py <driver>`（Rust runtime）在
2026-10-22 生成；hidpi/ 由 `scripts/drivers/dpi_shots.edn` 直接启动 viewer 生成。

## 1. driver 改写为 EDN 脚本并通过

| driver | 脚本 | 结果 | Lua 断言 → EDN 断言 |
|---|---|---|---|
| navigation | `navigation.edn` | PASS | fit、双击下钻+fit、Ctrl+滚轮光标锚缩放 1.1、空白拖拽平移、Ctrl+0 fit、Esc 返回、变换后单击命中、滚轮平移；另加 Declutter 五档循环且不动视图 |
| search | `search.edn` | PASS | Ctrl+F 聚焦、中文 textinput、深层/同名目标、搜索框截获画布键盘与指针、Down 显式选中、Enter 跨层定位+选中+卡片；另加中文粘贴、Esc 只关搜索框 |
| fonts | `fonts.edn` ×3 场景 | PASS | 来源 sarasa/env/system、角色字号、整张文案表+额外文本的字形全部可解析、1500x920 |
| font_parity | `font_parity.edn` / `font_parity_baseline.edn` | PASS | 1500x920 DPI1 缩放 1 下画布文字落在槽内；baseline 写 quality-diagnosis |
| dpi_window | `dpi_window.edn`（first 阶段，scale 1） | PASS | ui 缩放=环境缩放、字号、指针命中、窗口改 1100x700 后画布跟随、缩放 1/1.7 截图 |

断言关系式表达：`record-view` 记下视图，`assert-view` 比较（`:zoom-times`、`:cam-shift`、
`:anchor-fixed`、`:*-unchanged`、`:fitted`）。负例已验证：`UML_FONT_TEST_EXPECT=sarasa`
而实际为 system 时 viewer 以 1 退出，stderr 为对应断言消息。

**未等价移植、由后续 ticket 承接（Lua driver 仍可 `--runtime love` 运行）：**
- search：两个源码窗口并存/关闭其一 → #141 源码窗口；imgui.ini 隔离 → egui 不写 ini，无对应物。
- font_parity：640x700 类卡片无横向滚动、行高 20/18 → #141 类卡片。
- dpi_window：检查器侧栏 280/拖到 340/summary 卡/Regen 按钮 → #141；mail `:display`、
  `:quit-for-restart`、session 与 restart/legacy 阶段 → #142；imgui `FontGlobalScale` 无 egui 对应物。
- navigation：imgui 捕获阻断画布 → egui 等价物（搜索框截获）在 search.edn 断言。

## 2. 粘贴中文；IME 实际表现

- **已观察（driver）**：`:paste` 向 egui 注入 `Event::Paste("器")`，由 TextEdit 自身的
  输入路径插入，搜索框查询变为 `解析器`（search/search-pasted.edn）。`:text` 注入
  `Event::Text`，即 IME 提交文本走的同一入口。
- **未观察，需人工验证**：系统剪贴板的真实 Ctrl+V（egui 经 winit/arboard 读剪贴板）；
  WSLg 下的输入法组字（预编辑窗口位置、候选框、提交）。本次没有交互式输入法会话，
  因此 **IME 实际表现尚未记录**——需人工在 WSLg 中打开 Ctrl+F，用 Windows 中文输入法
  输入并把结果补写到此处。

## 3. 高分屏文字清晰（截图证据）

hidpi/scale2-zoom1.png：`UML_VIEWER_TEST_DPI_SCALE=2`，`:pixels-per-point 2`，
字形按 2 倍栅格化（CJK 标题、«interface»、α/I、无数据均清晰）；对照 scale1-zoom1.png。
窗口请求 3000x1840 物理像素，被 1920x1080 桌面限制为 960x540 点（与 Lua `window_size`
受桌面限制一致）。这是模拟缩放；**真实高分屏/每显示器缩放切换需人工验证**
（winit 上报 scale factor，egui 重新栅格化；本机显示器为 1 倍）。

## 其他

- 修复 #139 移植的输入双重变换：app.rs 先把指针转成世界坐标，`scene::events` 又转一次，
  相机非零或缩放≠1 时点击落空（navigation driver 暴露）。
- `commands::fit` 线性缩放后因字形步进取整可能仍超出预算 0.007px，egui 会折行；改为再测量收缩。
- 观察到、未核对：画布类名显示 id 末段（`Parser`、`A`）而非 `:name`（`解析器`、`中文类`）；
  是否与 LOVE viewer 一致尚未比对，不在本 ticket 修改。
