# #139 Rust viewer 曳光弹验收记录

日期：2026-10-22。原生 Ubuntu 24.04 WSL2 + WSLg（Weston 9 + Xwayland），
`DISPLAY=:0`、`WAYLAND_DISPLAY=wayland-0`；egui/eframe 0.37 + wgpu（GL 后端）。

## 验收标准核对

| 标准 | 结果 | 证据 |
|---|---|---|
| class_boxes、metrics driver 改写为 EDN 脚本，现有证据断言不改即通过 | 通过 | `metrics/`、`class_boxes/`：标记文件与 scene-EDN grep 一字未改（`:mu 1`/`:mu 9`/`:strokes`/`:tip`、等字节编辑、IR mtime 不变）；脚本在 `scripts/drivers/` |
| 截图证据：三档 LOD 各一张 | 通过 | `lod/lod-blocks.png`（zoom 0.1，块+包标题）、`lod/lod-frames.png`（0.25，框+类名+抽稀边）、`lod/lod-full.png`（1.0，完整细节）；由 `scripts/drivers/lod_shots.edn` 驱动 |
| D3D12 路径生效（非 llvmpipe） | 通过 | 每次运行的 `renderer.txt`：`renderer: D3D12 (NVIDIA GeForce GTX 1080 Ti)`、`backend: Gl`（`GALLIUM_DRIVER=d3d12`、`WGPU_BACKEND=gl` 生效） |
| 窗口装饰结论及理由 | 本机走 Wayland 无装饰 | 见下节 |
| 进程身份 JSON 结构沿用现有格式 | 通过 | `viewer-process-identity.json`：`{"pid":…,"started":"…","runtime":"rust","cwd":"…"}`，键序、无空格、无尾换行与 LOVE 版一致；`runtime` 为 `rust`（`uml_launch.verify_previous` 的 RUNTIMES 接受） |

空画布与文件缺失提示：`hints/waiting-display.png`（`--backend herdr` 等待
`:display`）、`hints/missing-file.png`（找不到 IR 文件），均为真实窗口截图。

复现：`cargo build --release` 后
`python3 -I scripts/run_viewer_driver.py metrics|class_boxes|lod_shots`。
LOVE 共存：`--runtime love` 仍走 `scripts/test_love_*.lua`（
`love-coexistence/` 里 metrics 的 LOVE 证据通过）。

## 窗口装饰结论

结论：**本机运行 Wayland 无装饰窗口**，顶条 28px 拖拽移动、边缘 6px
（角 18px）区域缩放，由 egui `ViewportCommand::StartDrag/BeginResize`
实现；X11 分支已按预检实现但本机未验证。

理由：

1. ADR 0008 的二选一约束：winit 的客户端装饰会让 Weston 断开连接，要
   标题栏就必须走 X11。
2. X11 预检（`DISPLAY` 存在 + `libxkbcommon-x11.so.0` 可加载）在本机
   失败：库未安装且本 agent 无 sudo，X11 装饰路径无法验证。
3. 无装饰 Wayland 路径已实测：窗口创建、D3D12 渲染、截图、driver 重放
   全部正常（本目录证据均为无装饰窗口产出）。

**留给人工验证**：安装 `libxkbcommon-x11-0` 后重跑 viewer，确认
`choose(x11_available())` 选 `X11Decorated`、标题栏/边框正常、拖拽与
缩放、IME 中文输入。无装饰路径的拖拽/缩放手感也需真人确认一次。

## 已知边界

- `--backend herdr` 进入等待 `:display` 的空画布；mailbox 读写是 #142，
  本 ticket 不轮询 mail。`./uml --runtime rust` 经 preflight（#138 的
  companion-preflight 已在 integration/spec-134 上）。
- session 读取/恢复已实现（`--restart`）；session 只在
  `:quit-for-restart` 时写，与 LOVE 一致，故需 #142 后才会产生。
- CJK 字体查找沿用 LOVE 顺序（`UML_CJK_FONT` → 项目 Sarasa → 系统
  Noto）；完整字体对齐是 #140。
