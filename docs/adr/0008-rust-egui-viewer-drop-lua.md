# 0008 改用 Rust + egui/eframe，整条链路去掉 Lua

**Status**: accepted

选型 map #124，ADR 由 #133 写成。本 ADR 取代 ADR 0005 中 LOVE + cimgui-love + Lua 的部分；
0005 放弃 JVM 和 Clojure 支持的决定继续有效。ADR 0007 定下的交付平台不变。

#106 在 WSL 下验收未通过。实测表明瓶颈在 viewer 每帧在 CPU 上重建全部几何，不在 WSLg 的软件渲染：
合成 500 类场景在 llvmpipe 和 D3D12（GTX 1080 Ti）下交互都只有 2.3 vs 2.2 fps；写实场景
任何 zoom 下首帧都会崩溃，原因是 cimgui 的 16 位顶点上限。在 WSLg + D3D12 上做的 egui/eframe 原型（#131）达标：2000 类场景
各相位 p95 ≤ 26.6 ms，idle CPU ≤ 1.7%；500 类 10 组里 9 组达 60 fps，未过的那组归因于 WSLg 的呈现尖峰。
即时模式加视口剔除、三档 LOD 已经够用。

## 决定

- viewer（渲染、交互、窗口）、场景核心、IR 生成与四个扫描器、companion 的 worker、recovery 和 preflight
  全部用 Rust 实现，编成一个二进制，用子命令区分角色（viewer、`ir`、companion worker 等）。
  仓库里不再保留 Lua 运行时代码；被分析的 Lua 项目照常支持，Lua 扫描器用 Rust 重写。
- 运行在 **WSLg + D3D12**。交付平台仍按 ADR 0007，companion、mailbox 和质量快照都不跨 Windows/WSL 边界。
- 对外数据契约保持 **EDN**：IR、policy（含 `:proposals`）、mail 队列、`.metrics/*.edn`。只有 viewer
  自己读写的文件（session 状态、缓存、进程身份）可以用 JSON。EDN 读写按现有 `edn.lua` 的语义移植，
  并用现有 EDN 文件做 round-trip 测试固定行为（键按字典序输出，支持 `\u` 转义）。是否以 clojure-reader 为底层
  由迁移 spec 决定；遇到不支持 `\u` 或 serde 静默丢数据的情况即否决。改写 policy 时和现在一样不保留注释。

## 平台约束

- 必须设 `GALLIUM_DRIVER=d3d12`，wgpu 后端还要设 `WGPU_BACKEND=gl`；漏设会落到 llvmpipe，2000 框探针从 154.7 fps 降到 31.9 fps。
- WSLg 帧率封顶 60 Hz；交互时进程 CPU 占 32–53%。
- winit 的客户端装饰会让 Weston 断开连接：要么去掉标题栏，要么安装 `libxkbcommon-x11-0` 走 X11，二选一由迁移 spec 决定。

## Considered Options

- **PixiJS v8 / Web 前端**：资料上 WebGL2 按需绘制可能达标，但**没有实测**。egui 原型达标后由维护者直接选定 egui，
  没有做跨栈对照（#132 移出范围）。以后再有人提 Web，应当从实测做起。
- **iced 0.14**：还没到 1.0。**Cytoscape.js**：不支持正交边和空心箭头（#129）。
- **优化现有 LOVE 方案**（VtxOffset、Canvas 缓存、LOD）：维护者判断不值得做；换 GPU 后端的实测结果也没有差别。
- **Godot 等游戏引擎**：现有代码几乎无法复用。
- **无头角色保留独立的 LuaJIT 宿主**（#130 的原决定）：无头角色以 I/O 为主，换语言并不能提速；放弃它是为了只剩一套工具链：
  安装器不再供给 LOVE、LuaJIT、Lua 5.4、lfs、luacov/cluacov 和 cimgui，双解释器约束也随之消失。
- **Windows 原生 egui，companion 留在 WSL**：能突破 60 Hz，但要推翻 ADR 0007，并处理 9P 文件监听、路径和进程身份。
  WSLg 已经达标，因此不做，保留为将来可以重开的选项。
- **对外契约改成 JSON**：三个质量工具输出 EDN，mail 协议也由 agent 按 EDN 书写，改格式要另起一个项目。

## Consequences

- 有意推翻 ADR 0005 否决 Dear ImGui C++ 时的理由（「纯逻辑要设计静态类型」）：实测证明性能是刚需，
  因此接受场景核心约 2.7k 行动态 Lua 移植到 Rust 的成本，这比移植到 TS 更贵。移植后的 layout/route 必须严格通过
  JVM goldens（误差 ≤ 1e-6），可以修掉 `prepare_scene` 的 O(E·N)，但几何输出不能变。JVM goldens 是
  `fixtures/jvm-goldens/` 下的固定 EDN 对照文件，由已删除的 JVM 实现产出；测试只读这些文件，不需要 JVM。
- ADR 0007 adapter 表中的 POSIX 契约（字面 argv、显式 envp、`flock`、`rename`、`statx`、starttime 进程身份、
  Herdr socket 长度）照旧，实现换成 Rust。「LOVE 报告 `Linux`」这条平台判定在迁移 spec 中改写成等价的检查。
- 本仓库自举的质量分析改为 `:lang :rust`。crapper、mutator、dryer 都支持 Rust，但 Rust mutation 每个变异体都要
  重新编译，`./uml mutate` 会明显变慢；这是已接受的代价。
- 迁移实现由单独的 spec 承担；新旧实现并存到功能对等后再删除 Lua 实现。
