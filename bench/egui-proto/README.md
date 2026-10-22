# PROTOTYPE：egui/eframe 渲染基准（#131，一次性代码）

服务于 map「viewer 技术栈选型：替代 LOVE + Lua」(#124) 的 ticket「egui 最小渲染原型实测（WSLg d3d12）」(#131)。
只回答一个问题：egui/eframe 渲染 `bench/` 写实 500 / 2000 类场景，能否达到 map 的性能目标。不是产品代码。

## 运行

```sh
cd bench/egui-proto && cargo build --release        # Rust 1.99，eframe 0.37（wgpu 后端）
bench/run_egui.sh bench/scenes/synthetic-500.scene.json bench/results/egui-500-d3d12-run1
```

`run_egui.sh` 设置 `GALLIUM_DRIVER=d3d12`、`WGPU_BACKEND=gl`。环境变量：`BENCH_SHOTS=1` 只截图，
`BENCH_CJK=1` 截图对准中文类名，`BENCH_ONLY=<相位>` 只跑一个相位，`BENCH_TEXT_QUANT=0` 文字字号不取整，
`BENCH_DECIM=<px>` 中档 LOD 的边折线抽稀阈值（默认 8），`BENCH_FEATHER=0` 关闭抗锯齿，`BENCH_VSYNC=0`。

## 实现要点

- egui painter 即时模式，每帧只为视口内的类和边（预计算世界包围盒）生成 shape。没有用 `CallbackTrait`，
  因为即时模式已经够用。
- LOD 分三档：zoom < 0.15 只画色块（用描边色填充）和固定 13 px 的包标题；0.15–0.4 加描边、C/M 色点和抽稀到 8 px 的边；
  ≥ 0.4 画全部内容：文字、分隔线、箭头和无数据虚线框。
- 文字字号按 1 px 取整，让字形图集里的字号种类有界。
- 空闲时不请求重绘，idle 相位只靠一次定时唤醒结束。
- 协议相位、轨迹和指标与 `love_protocol_driver.lua` 一致；另外记录 `app_cpu_ms_p95`，即 eframe 报告的每帧应用 + 三角化 + 提交耗时，作为 60 Hz 封顶下的余量指标。

## 结果（Ryzen 5 5500 / GTX 1080 Ti，WSLg，renderer `D3D12 (NVIDIA GeForce GTX 1080 Ti)`，wgpu GL 后端，Mesa 25.2.8）

每格：fps / p95 ms / 进程 CPU% / 应用 CPU p95 ms。

| 场景 | 相位 | run1 | run2 | 判定 |
|---|---|---|---|---|
| 500 | idle | — / — / 1.7% | — / — / 0.4% | ✓ CPU ≤ 2% |
| 500 | pan-detail | **57.5** / 17.6 / 45 / 5.2 | 59.1 / 17.1 / 40 / 4.6 | run1 mean 略低于 58 |
| 500 | zoom-detail | 58.3 / 17.3 / 45 / 5.4 | 59.1 / 17.2 / 41 / 5.5 | ✓ |
| 500 | pan-fit | 58.9 / 17.3 / 36 / 1.1 | 59.3 / 17.2 / 32 / 1.0 | ✓ |
| 500 | zoom-fit | 58.3 / 17.4 / 44 / 8.8 | 59.0 / 17.2 / 42 / 9.1 | ✓ |
| 2000 | idle | — / — / 0.5% | — / — / 0.4% | ✓ |
| 2000 | pan-detail | 58.0 / 17.4 / 49 / 5.0 | 56.2 / 26.6 / 53 / 5.9 | ✓（p95 ≤ 33.3） |
| 2000 | zoom-detail | 58.8 / 17.3 / 53 / 7.4 | 59.2 / 17.1 / 51 / 6.9 | ✓ |
| 2000 | pan-fit | 58.4 / 17.4 / 37 / 1.8 | 59.2 / 17.2 / 35 / 1.8 | ✓ |
| 2000 | zoom-fit | 58.9 / 17.3 / 36 / 2.7 | 59.4 / 17.1 / 33 / 2.6 | ✓ |

idle 相位只有 1 帧（定时唤醒），所以只看 CPU 一项。

### 诊断实验（单相位，`BENCH_ONLY`）

| 变体 | 相位 | fps / p95 ms / max ms / 应用 CPU p95 |
|---|---|---|
| 中档边抽稀 3 px（首版） | 500 zoom-fit | 55.4 / 27.6 / 35 / 11.8 ✗ |
| 3 px + 关闭抗锯齿 | 500 zoom-fit | 59.4 / 17.1 / 17 / 7.4 |
| 8 px（默认） | 500 zoom-fit | 59.1 / 17.1 / 28 / 8.2 |
| 字号不取整 | 2000 zoom-detail | 54.7 / 27.4 / **134** / 11.6 ✗ |
| 字号取整 1 px（默认） | 2000 zoom-detail | 59.4 / 17.2 / 29 / 6.4 |

首版 vsync 关（`AutoNoVsync`）与开结果相同：WSLg 下帧率封顶 60 Hz，无法测出超过 60 fps 的余量。

## 观察

- **WSLg Wayland 下 winit 客户端装饰会断连**：带标题栏时 Weston 很快重置连接
  （`Io error: Connection reset by peer`，进程退出），去掉装饰（`with_decorations(false)`）后稳定。
  走 X11 需要 `libxkbcommon-x11-0`，本机没装。
- **交互时的进程 CPU 偏高**：每帧应用侧只有 1–9 ms，但交互相位的进程 CPU 是 32–53%。pan-fit 每帧应用只有 1 ms，CPU 仍有 35%，
  多出来的部分应该是 Mesa d3d12 GL 驱动和 WSLg 呈现路径的开销。
- **偶发帧尖峰**（27–45 ms）各相位都有，连应用只花 1 ms 的 pan-fit 也有，判断来自 WSLg 呈现路径。
  500 run1 pan-detail 因此 mean 57.5 fps，没过 58 的门槛。
- 中文：Noto Sans CJK SC 作回退字体，类名（如「检件」）正常显示；见 `results/egui-500-cjk-shots/`。
- 文字清晰度：每个字号都按原生像素栅格化，zoom 2 下依然锐利（`results/egui-500-shots/shot-zoom-2.png`）。
  前提是字号取整，否则会出现 100 ms 级的尖峰。
- 空心三角箭头、虚线框和违规红边都能直接画出来。egui 没有虚线矩形图元，用 `Shape::dashed_line` 拼成。
- 构建 shape 的 p95 ≤ 3 ms，大头在 egui 的抗锯齿三角化，中档 LOD 的边最贵。如果真实图比合成图更密，
  中档 LOD 是第一个需要优化的地方（比如缓存三角化结果，或改走 `CallbackTrait`）。
