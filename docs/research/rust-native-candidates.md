# Rust 原生候选能否达到大图性能目标

wayfinder 研究票 [#125](https://github.com/qinyuanj/uml-viewer/issues/125)，属于 map
[#124](https://github.com/qinyuanj/uml-viewer/issues/124)。资料检索与实测日期均为
2026-10-21。

## 结论

- **可以。** 没有一手资料或可复现基准直接证明某个候选在 2000 类 UML 图上达标，但以下三点在架构上支持达标：
  1. GPU 路径的上限远高于目标；
  2. egui 和 iced 都只在有输入或动画时重绘，空闲时 CPU 接近 0；
  3. 两者都提供了绕开逐帧 CPU 重建几何的途径。

  是否达标要靠原型实测确认。
- **推荐进入原型：**
  1. **egui + eframe（wgpu 或 glow）**
  2. **iced 0.14（wgpu）**
- **不推荐：**
  - Vello：只是渲染器，不是 GUI。
  - Slint：声明式元素树不适合 2000 节点的自由画布，也没有相关证据。
  - Makepad：最近一次发布停在 2025-05；中文 IME 修复只在 `dev` 分支。
- **头号风险在平台，不在框架。** WSLg 默认走 llvmpipe 软件渲染，#106 的基线就是在 llvmpipe 上测的。Vulkan 在 WSLg 里只有 lavapipe（也是软件渲染）。任何候选在 WSLg 下都必须显式走 D3D12 GL 路径，否则会重复基线的结果。Windows IME 的中文输入在 WSLg 里不可用，只能用 WSL 内的 fcitx5/ibus。

## 前提：WSLg 的 GPU 路径（本机实测）

### 基线跑在软件渲染上

`acceptance-evidence/106-wsl/performance/synthetic-500-benchmark.edn` 的
`:environment` 记录了 `:device "llvmpipe (LLVM 20.1.2, 256 bits)"`。合成 500 类
2 fps 的基线是在 CPU 光栅化下测得的。

### 默认不会选 D3D12

本机（Ubuntu 24.04、Mesa 25.2.8、GTX 1080 Ti、WSL 内核 6.18）上，
`d3d12_dri.so` 已经安装，但 GL 默认仍选 llvmpipe。设置 `GALLIUM_DRIVER=d3d12`
后得到 `OpenGL 4.6 … D3D12 (NVIDIA GeForce GTX 1080 Ti)`。这与 WSLg 的公开问题
一致：[microsoft/wslg#1332](https://github.com/microsoft/wslg/issues/1332)
「D3D12 not used unless GALLIUM_DRIVER set」，至 2026-09 仍未关闭。

### Vulkan 在 WSLg 里只有软件实现

`/usr/share/vulkan/icd.d/` 中只有 lavapipe（`lvp_icd.json`）等 ICD，没有 Dozen
（`dzn`，即 D3D12 上的 Vulkan 实现）。另外：

- [wslg#40](https://github.com/microsoft/wslg/issues/40)「Add support for Vulkan」未关闭；
- [wslg#1492](https://github.com/microsoft/wslg/issues/1492) 记录 Dozen 在 NVIDIA 上 `vkCreateDevice` 段错误。

wgpu 在 Linux 上把 Vulkan 列为一级后端，GL 列为「Downlevel/Best Effort」
（[wgpu README](https://github.com/gfx-rs/wgpu#supported-platforms)；
[`Backends::PRIMARY`/`SECONDARY`](https://github.com/gfx-rs/wgpu/blob/trunk/wgpu-types/src/backend.rs)）。
因此 wgpu 系框架在 WSLg 上必须设置 `WGPU_BACKEND=gl`
（[README 环境变量](https://github.com/gfx-rs/wgpu#environment-variables)），并配合
`GALLIUM_DRIVER=d3d12`。

> 推断：wgpu 默认会不会选中 lavapipe，没有在本机实测（本机没有 Rust 工具链），需在原型里确认。

### 呈现拷贝开销

WSLg 的 vGPU 呈现要经过系统内存拷贝。按官方说法，在极高帧率（600 fps）下开销可达
50%，但仍然「very significant」优于软件渲染
（[WSLg README · vGPU](https://github.com/microsoft/wslg#wslg-architecture-overview)）。
[wslg#1498](https://github.com/microsoft/wslg/issues/1498)（2026-08）报告：在 Intel
iGPU 上，这一回读让 1280×720 掉到约 2.8 fps。该问题与驱动和 GPU 有关，在本机
NVIDIA 上没有复现（见下表）。

### 本机探针

用一个一次性的 LOVE 场景测量 WSLg 本身的上限，与框架无关：

- 1500×920 窗口，vsync 关闭，与 #106 相同；
- 2000 个矩形框、2000 个类名文本、2000 条 4 点折线；
- 连续平移和缩放，测 8 s。

| 场景 | llvmpipe（默认） | D3D12（`GALLIUM_DRIVER=d3d12`） |
|---|---|---|
| 空帧 | 282.8 fps，p95 4.50 ms | 199.5 fps，p95 6.69 ms |
| 2000 框 + 文本 + 折线 | 31.9 fps，p95 38.96 ms | **154.7 fps，p95 8.31 ms** |

D3D12 下空帧比 llvmpipe 慢，正是上面所说的呈现拷贝开销。即使场景很朴素，D3D12 下
也比目标（p95 ≥ 30 fps）高约 5 倍。所以 #106 的 2 fps 主要来自两部分：

- 应用侧逐帧的 CPU 工作；
- 软件光栅化。

它不代表 WSLg 的上限。探针脚本见附录，可复现。

## 评估维度

map 的性能目标：

- 2000 类图平移/缩放 p95 ≥ 30 fps；
- 空闲时几乎不占 CPU；
- 500 类稳定 60 fps。

满足这些目标需要：

1. 只在有输入或动画时重绘（空闲 CPU）；
2. 平移/缩放时不逐帧在 CPU 上重建全部几何，或者重建足够便宜。手段可以是 GPU 变换、缓存几何、视口裁剪或 LOD；
3. 文本按 glyph atlas 批量绘制。

其余维度：CJK 文本与 IME、剪贴板、DPI、Windows 原生与 WSLg 支持、成熟度。

## 候选

### egui + eframe（推荐）

**版本与活跃度**

- 0.37.0 于 2026-10-15 发布，此前 0.36.x 在 8–9 月发布（[releases](https://github.com/emilk/egui/releases)）。
- 由 Rerun 资助（[README](https://github.com/emilk/egui#readme)）。

**空闲 CPU**

README 写明「only repaints when there is interaction … or an animation, so if your
app is idle, no CPU is wasted」。典型帧耗时 1–2 ms
（[Why immediate mode · CPU usage](https://github.com/emilk/egui#cpu-usage)）。

**大量图元**

- 即时模式每帧重新布局并在 CPU 上做三角化。README 自己承认，非常大的界面会给 CPU 带来负担，建议「only lay out the part that is in view」。
- 仓库自带 tessellation 基准（[`epaint/benches/benchmark.rs`](https://github.com/emilk/egui/blob/main/crates/epaint/benches/benchmark.rs)，含 `tessellate_circles_100k`），但没有公布结果。
- 如果 2000 类全部可见时三角化太慢，可以用 `egui_wgpu::CallbackTrait` 把图层交给自写的 wgpu 管线，用实例化绘制矩形和折线，egui 只负责面板（[`egui-wgpu/src/renderer.rs`](https://github.com/emilk/egui/blob/main/crates/egui-wgpu/src/renderer.rs)）。

**CJK 与 IME**

- 0.37 起可以通过 `fontique` 回退到系统字体，渲染任意字形，包括非拉丁文字（[0.37.0 release notes](https://github.com/emilk/egui/releases/tag/0.37.0)）。
- 2026-06 合入 IME 组字的光标和分段视觉（[PR #8083](https://github.com/emilk/egui/pull/8083)）。
- 未关闭的 IME 问题：
  - [#7975](https://github.com/emilk/egui/issues/7975)：Linux X11 + fcitx5 下预编辑文字不在 `TextEdit` 内；
  - [#3532](https://github.com/emilk/egui/issues/3532)：IME 输入异常。

**剪贴板**

`egui-winit` 的 `clipboard` 特性使用 `arboard` 和 `smithay-clipboard`，支持 Wayland 和
X11，默认开启（[egui-winit Cargo.toml](https://github.com/emilk/egui/blob/main/crates/egui-winit/Cargo.toml)）。

**DPI**

使用 winit 的 scale factor，即 `pixels_per_point`。

**后端**

eframe 默认 `wgpu`，也可以选 `glow`（OpenGL）
（[eframe Cargo.toml](https://github.com/emilk/egui/blob/main/crates/eframe/Cargo.toml)）。
在 WSLg 上，`glow` 直接走 Mesa GL，可以绕开 wgpu 的后端选择问题。

**结论**

成熟、活跃，空闲 CPU 有保证。风险在于 2000 类全部可见时的 CPU 三角化，原型要量这一点。

### iced 0.14（推荐）

**版本与活跃度**

- 0.14.0 于 2025-12-07 发布，之后没有新版本。
- master 仍然活跃，最近提交在 2026-10-20（[commits](https://github.com/iced-rs/iced/commits/master)）。
- 尚未到 1.0，API 有破坏性变化的风险。

**空闲 CPU**

0.14 加入了「Reactive rendering」
（[#2662](https://github.com/iced-rs/iced/pull/2662)，[0.14.0 release](https://github.com/iced-rs/iced/releases/tag/0.14.0)）。

**大量图元**

- `canvas::Cache` 存放已生成的 `Geometry`：「will not redraw its geometry unless the dimensions of its layer change or it is explicitly cleared」（[canvas.rs](https://github.com/iced-rs/iced/blob/0.14.0/widget/src/canvas.rs)）。
- 渲染器提供 `with_transformation`（[core/renderer.rs](https://github.com/iced-rs/iced/blob/0.14.0/core/src/renderer.rs)）。缓存的几何加上 GPU 变换，平移和缩放时就可以不重新三角化。
- 0.14 还加入了 `draw_mesh_cache`、图层合并和 `column`/`row` 图元裁剪。
- 缩放后缓存文字的清晰度还没有一手证据，需要原型确认。

**CJK 与 IME**

- 文本由 `cosmic-text` 0.15 渲染。
- 0.14 加入「Input method support」（[#2777](https://github.com/iced-rs/iced/pull/2777)），并修复了预编辑位置和候选框位置问题。
- 未关闭：[#3189](https://github.com/iced-rs/iced/issues/3189)，Windows 日文 IME 转换弹窗异常。

**后端**

- wgpu（0.14 默认 `PowerPreference::HighPerformance`）和 tiny-skia（CPU 渲染）。
- 0.14 修复了若干 wgpu OpenGL 后端的纹理问题（[#2919](https://github.com/iced-rs/iced/pull/2919)、[#3016](https://github.com/iced-rs/iced/pull/3016)），说明 GL 路径有人在用，也有人在修。

**结论**

保留模式加几何缓存，从机制上最贴合「缓存一次，平移缩放只改变换」。风险在于发布节奏慢、API 不稳定。

### Vello（不推荐单独入围）

- 自述是「2D vector rendering」项目，包含三套渲染器（[README](https://github.com/linebender/vello#readme)）：
  - `vello_cpu`：最成熟；
  - `vello_gpu`：不需要计算着色器，「intended to become the primary renderer … as it matures」；
  - 原始的 compute 渲染器：「experimental」。
- v0.11.0 于 2026-10-02 发布。
- 它不提供窗口、事件、IME、剪贴板和控件，这些都要自己用 winit、parley 等拼装，工作量最大。
- 可以作为后续渲染后端：Slint 已经有实验性的 `renderer-vello`（[Slint CHANGELOG](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md)）。

### Slint（不推荐）

**版本与许可**

- 1.18.1 于 2026-09-21 发布，维护活跃。
- 许可三选一：GPLv3、Royalty-free 2.0、商业（[README · License](https://github.com/slint-ui/slint#license)）。

**渲染与能力**

- 渲染器：femtovg（GL ES 2.0）、skia、software，以及实验性的 vello。
- 支持 `Path`、`transform-scale` 和局部重绘，也有 IME 修复（同上 CHANGELOG）。

**不推荐的原因**

- 这是一个声明式元素树框架，面向控件界面。
- 把 2000 个类框、成员行和依赖边做成 repeater 元素，能不能在缩放时保持 30 fps，没有找到一手证据或基准。
- 退路是用 rendering notifier 自己画底层，但那样就和 egui 或 iced 的自定义渲染没有区别了，还多了一套 DSL 和许可选择。

### Makepad（不推荐）

**版本**

crates.io 上 `makepad-widgets` 最新 1.0.0，发布于 2025-05-13。master 当天仍有推送，但最近 17 个月没有发布。

**IME**

- Windows IME 代码在 `platform/src/os/windows/win32_window.rs`。
- Linux 中文输入问题 [#591](https://github.com/makepad/makepad/issues/591) 和 fcitx 问题 [#795](https://github.com/makepad/makepad/issues/795) 于 2026-09-28 关闭，维护者的说明是「fixed on current `dev`: linux IME for X11 (XIM) and Wayland (text-input-v3)」，也就是说还没进入任何发布版。

**其他**

自带着色器 DSL 和脚本引擎，README 以「AI-accelerated application and game development environment」定位（[README](https://github.com/makepad/makepad#readme)）。它的 GPU 性能取向符合需求，但要跟踪 `dev` 分支，生态也窄，风险最高。

### 未列入

GPUI（Zed）在 crates.io 上最新为 0.2.2，发布于 2025-10-22，独立使用的文档和稳定性承诺有限，本票不评估。

## 平台共性

这些问题对所有候选都一样，由部署形态决定。

**WSLg 的 GPU**

- 必须设置 `GALLIUM_DRIVER=d3d12`；wgpu 系还要加 `WGPU_BACKEND=gl`。
- Intel iGPU 机器可能遇到 wslg#1498 的呈现回读瓶颈。

**WSLg 的 IME**

- WSLg 目前不把 Windows IME 转给 Linux 窗口，需要在 WSL 内手动配置 ibus 或 fcitx5（[wslg#9](https://github.com/microsoft/wslg/issues/9)，未关闭）。
- 2026-10 有社区原型用 MS-RDPETXT 打通了 Windows 中文输入法，但尚未进入官方。

**WSLg 的剪贴板**

Weston RDP 后端集成了剪贴板（[WSLg README](https://github.com/microsoft/wslg#wslg-architecture-overview)），候选只需要支持 Wayland 或 X11 剪贴板。

**Windows 原生**

- egui 和 iced 都基于 winit 加 wgpu，在 Windows 上走 DX12 或 Vulkan，都是一级后端。
- IME 走 Win32，绕开上面所有 WSLg 问题。
- 代价是 viewer 与 companion 的 mailbox 要跨 9P，这是 map 里「Not yet specified」中的部署形态问题。

## 对比

| 维度 | egui/eframe | iced 0.14 | Vello | Slint | Makepad |
|---|---|---|---|---|---|
| 空闲重绘 | 按需 | 反应式（0.14） | 自建 | 按需、局部重绘 | 按需 |
| 平移/缩放大图 | 每帧 CPU 三角化；可改用 wgpu 回调 | Cache + GPU 变换 | 渲染器本身 | 元素树，无证据 | GPU，自定义 DSL |
| CJK 字体 | 0.37 系统字体回退 | cosmic-text | glifo/parley | 支持 | 支持 |
| IME | 可用，有 X11/fcitx5 问题 | 0.14 加入 | 自建 | 支持 | 仅 `dev` |
| WSLg 走 GPU | glow 或 `WGPU_BACKEND=gl` | `WGPU_BACKEND=gl` | 同 wgpu | femtovg GL | GL |
| 最近发布 | 2026-10-15 | 2025-12-07 | 2026-10-02 | 2026-09-21 | 2025-05-13 |

## 给原型的建议

1. 两个原型使用同一个合成场景：2000 类 / 500 类，框、成员行、依赖折线。窗口 1500×920，vsync 关闭，按 #106 的 idle / interaction 相位记录 fps、mean、p95 和空闲 CPU。
2. WSLg 运行时固定 `GALLIUM_DRIVER=d3d12`（wgpu 系再加 `WGPU_BACKEND=gl`），并把 renderer 和 adapter 字符串写进结果，避免再测到软件渲染。条件允许时在 Windows 原生再测一遍。
3. egui 先用原生 `Painter` 加视口裁剪；p95 不达标再换 `CallbackTrait` 实例化。iced 用 `canvas::Cache` 加 `with_transformation`，同时检查缩放时文字是否清晰。

## 附录：WSLg 呈现探针（一次性脚本，非产品代码）

运行方式：`PROBE_MODE=scene|clear [GALLIUM_DRIVER=d3d12] love <dir>`，需要 LOVE 11.5。

```lua
local mode = os.getenv("PROBE_MODE") or "scene"
local N = tonumber(os.getenv("PROBE_N") or "2000")
local seconds = tonumber(os.getenv("PROBE_SECONDS") or "8")
local deltas, t0, elapsed = {}, nil, 0
local boxes, lines = {}, {}
local font, texts
function love.load()
  love.window.setMode(1500, 920, { vsync = 0, resizable = false })
  local cols = math.ceil(math.sqrt(N))
  for i = 1, N do
    local c, r = (i - 1) % cols, math.floor((i - 1) / cols)
    boxes[i] = { x = c * 34, y = r * 20, w = 30, h = 16 }
  end
  for i = 1, N do
    local a, b = boxes[i], boxes[(i * 7) % N + 1]
    lines[i] = { a.x + 15, a.y + 16, a.x + 15, (a.y + b.y) / 2 + 8,
                 b.x + 15, (a.y + b.y) / 2 + 8, b.x + 15, b.y }
  end
  font = love.graphics.newFont(8)
  texts = love.graphics.newText(font)
  for i = 1, N do texts:add("C" .. i, boxes[i].x + 2, boxes[i].y + 3) end
  print("RENDERER", love.graphics.getRendererInfo())
end
function love.update(dt)
  if t0 == nil then t0 = love.timer.getTime() return end
  elapsed = elapsed + dt
  if elapsed > 1 then deltas[#deltas + 1] = dt * 1000 end
  if elapsed > seconds + 1 then
    table.sort(deltas)
    local sum = 0
    for _, d in ipairs(deltas) do sum = sum + d end
    local mean = sum / #deltas
    local p95 = deltas[math.max(1, math.floor(#deltas * 0.95))]
    print(string.format("RESULT mode=%s n=%d frames=%d fps=%.1f mean_ms=%.2f p95_ms=%.2f",
      mode, N, #deltas, 1000 / mean, mean, p95))
    love.event.quit()
  end
end
function love.draw()
  love.graphics.clear(0.1, 0.1, 0.12)
  if mode == "clear" then return end
  local s = 0.75 + 0.25 * math.sin(elapsed)
  love.graphics.push()
  love.graphics.translate(20 + 40 * math.sin(elapsed * 2), 20)
  love.graphics.scale(s, s)
  love.graphics.setColor(0.4, 0.6, 0.9)
  for i = 1, N do local b = boxes[i]; love.graphics.rectangle("line", b.x, b.y, b.w, b.h) end
  love.graphics.setColor(0.6, 0.6, 0.6, 0.5)
  for i = 1, N do love.graphics.line(lines[i]) end
  love.graphics.setColor(1, 1, 1)
  love.graphics.draw(texts)
  love.graphics.pop()
end
```
