# Web 前端候选能否达到大图性能目标

研究票 [#126](https://github.com/qinyuanj/uml-viewer/issues/126)，map
[#124](https://github.com/qinyuanj/uml-viewer/issues/124)。资料核对日期 2026-10-21。
只引一手资料（官方文档、仓库 issue / release、厂商博客）；凡是本文自己的推断都标
**推断**，必须由原型实测确认。

## 结论

资料层面，GPU 渲染（WebGL2，可选 WebGPU）加按需绘制的 Web 前端有能力达到 map 的目标
（2000 类平移/缩放 p95 ≥ 30 fps，500 类稳定 60 fps，空闲几乎不占 CPU）；纯 Canvas2D
和 DOM 方案没有资料支撑。没有任何一手资料给出“UML 类卡片 + 中文多行文字 + 折线边”
这一组合的实测帧率，所以达标与否只能由原型回答。

推荐进入原型的两个候选：

1. **PixiJS v8 自绘**（WebGL2/WebGPU），运行在 Windows 的 Edge/Chrome 中。
2. **Cytoscape.js ≥ 3.31 的 WebGL 渲染器**，同样运行在 Windows 浏览器中。

两者都配一个 **WSL 内的桥接进程**：用 inotify 监听 ext4 上的 IR / 快照 / mailbox，经
localhost 的 HTTP + WebSocket 推给前端。Windows 侧不直接读写或监听 `\\wsl$` 文件。

不推荐：React Flow（DOM）、Sigma.js 4（刚发布的重写版本，模型是点而不是多行卡片）、
G6 5（缺少一手规模数据），以及任何纯 Canvas2D 方案。

## 基线（对照）

`acceptance-evidence/106-wsl/README.md`「性能」一节和
`performance/synthetic-500-summary.txt`：在 WSLg 下运行 LOVE，合成 500 类场景，
idle 9.77 fps，interaction 2.02 fps（p95 3668 ms）。目标比现状高一到两个数量级，所以候选的
渲染路径必须让平移/缩放只改变一个变换，而不是每帧重绘全部图元。

## 场景特征：为什么“图库”不一定合适

UML 类图里的元素是 **矩形卡片 + 多行文字（类名、成员）+ 折线/正交边**，规模是 500~2000 个
节点，边数与节点数同一量级。平移/缩放期间，内容本身几乎不变。

**推断**：最有利的渲染方式是每张卡片栅格化一次成纹理，再由 GPU 批量绘制；平移/缩放
只改一个 uniform 或变换矩阵；没有输入、没有数据变化时不请求下一帧。主要风险在于缩放
跨越清晰度阈值时需要重新栅格化文字，这会产生帧尖峰，正好打在 p95 上。

## 候选逐项

### PixiJS v8 自绘（推荐 1）

- 版本：`pixi.js` 8.22.0（npm，2026-10-01）。
- 渲染：WebGL 或 WebGPU，由同一套 API 抽象；渲染采用保留模式
  （[Render Loop](https://pixijs.com/8.x/guides/concepts/render-loop)）。
- 平移/缩放：Render Group 会把整组内容的变换、tint、alpha 交给 GPU，“以最小 CPU 代价移动
  整组”（[Render Groups](https://pixijs.com/8.x/guides/concepts/render-groups)）。正好
  对应“整张图作为一个 world 容器”。
- 大量矩形：小 Graphics 对象会合批，速度与 Sprite 相当；数百个复杂 Graphics 会变慢，此时
  建议转成纹理；剔除默认关闭，可按对象开启
  （[Performance Tips](https://pixijs.com/8.x/guides/concepts/performance-tips)）。
- 文字（关键风险）：
  - `Text` 用浏览器原生文字引擎栅格化成纹理，适合“偶尔变化”的文本，但官方提示不适合
    “数百个实例”且每帧改动的场景
    （[Text](https://pixijs.com/8.x/guides/components/scene-objects/text)）。
  - `BitmapText` 适合上万个文本，但官方明确写到 **CJK 大字符集不现实**（受纹理尺寸
    限制），建议改用 `Text`/`HTMLText`
    （[Bitmap Text](https://pixijs.com/8.x/guides/components/scene-objects/text/bitmap)）。
  - **推断**：可行的组合是每张卡片用 `Text` 栅格化一次（中文由浏览器字体栈处理），然后
    缓存为纹理或 Sprite。另一条路是只把图中实际出现的字符（有限集合）动态生成
    BitmapFont。两条路都要在原型里测显存和缩放时的重栅格化尖峰。
- 空闲 CPU：Ticker 默认基于 `requestAnimationFrame` 持续运行（Render Loop 文档）。要做到
  空闲近零，需要停掉 ticker、改为脏标记触发 `render()`（**推断**，属于常见做法，原型需要
  实测）。
- 成熟度：v8 已发布到 8.22，文档完整；但它不是图库，布局、命中测试、边的路由都由我们
  自己实现（现有 IR 已经给出坐标，所以布局不是新增负担）。

### Cytoscape.js + WebGL 渲染器（推荐 2）

- 版本：`cytoscape` 3.34.4（npm，2026-10-15）；WebGL 渲染器自 3.31 起以 preview 形式
  提供（[WebGL Renderer Preview, 2025-01-13](https://blog.js.cytoscape.org/2025/01/13/webgl-preview/)）。
- 渲染方式：节点和标签先由 Canvas 渲染器画进 sprite sheet（纹理图集），再交给 GPU 反复
  绘制。**所有节点和标签样式都支持**，所以中文文字走浏览器 Canvas 文字，没有字形
  图集的问题。边画成细长矩形（最多 4 个三角形）。
- 一手性能数据（厂商自测，M1 MacBook Pro + Chrome，作者注明“YMMV”）：1200 个节点、
  16000 条边，Canvas 约 20 fps，WebGL 超过 100 fps；3200 个节点、68000 条边，从 3 fps
  提升到 10 fps。我们的边数远少于这两个样本。
- 已知限制（同一来源）：WebGL 模式 **不支持 taxi/segmented 边**（退化为 bezier），也不
  支持虚线、多个边标签和非三角形箭头。UML 的正交折线和空心三角（继承）箭头因此受影响。
  **推断**：可以把边改成直线或 bezier，或者把箭头另画在叠加层里，但这会改变视觉语义，
  需要用户确认。
- 首次加载：生成 sprite sheet 需要时间，preview 版本没有进度条。启动时间只记录、不设门槛。
- Canvas 渲染器的上限：单线程、逐元素按 z 序绘制，元素越多帧率越低（同一篇博客）。所以
  只有 WebGL 模式值得进入原型。
- 成熟度：项目已有十余年，持续发布；WebGL 仍标注为 preview。

### 不推荐的候选

| 候选 | 证据 | 不推荐的理由 |
|---|---|---|
| React Flow（`@xyflow/react` 12.12.0） | 官方 [Performance](https://reactflow.dev/learn/advanced-use/performance) 页面的建议都是减少重渲染、折叠大树、隐藏节点、简化 CSS | 节点是 DOM 元素。官方没有给出规模数据，应对大图的办法是“少画”。2000 张多行卡片在 DOM 里平移缩放，没有达标依据。可以考虑用来做 inspector 等 DOM 面板 |
| Sigma.js 4（`sigma` 4.0.3） | [4.0.0 release](https://github.com/jacomyal/sigma.js/releases/tag/sigma%404.0.0)（2026-10-08）：整体重写 programs，标签改为 WebGL SDF 字形，放弃 WebGL1；同期 issue：[#1552](https://github.com/jacomyal/sigma.js/issues/1552) SDF 图集溢出导致字形缺失，[#1553](https://github.com/jacomyal/sigma.js/issues/1553) 动画中每帧重建标签缓冲，[#1562](https://github.com/jacomyal/sigma.js/issues/1562) 标签着色器 INVALID_OPERATION | 主版本刚发布两周，SDF 图集遇到大字符集正是 CJK 风险点；模型是带单行标签的点，不是多行类卡片 |
| G6 5（`@antv/g6` 5.1.1，2026-05-08） | [renderer 文档](https://g6.antv.antgroup.com/en/manual/further-reading/renderer)：默认 Canvas，可切到 `@antv/g-webgl` | 没有找到官方给出的大图帧率数据；抽象层较厚（G 引擎），自己的绘制路径难以控制 |
| 任何纯 Canvas2D / SVG 方案 | Cytoscape 博客对单线程 Canvas 的分析 | 每帧重画全部图元，正好是现状 LOVE 方案的瓶颈结构 |

WebGPU：Chrome 113 起在 Windows 默认开启（[Chrome ships WebGPU](https://developer.chrome.com/blog/webgpu-release)）。
PixiJS 可以选用 WebGPU；Cytoscape 明确选择 WebGL，理由是 WebGPU 跨浏览器支持不足。
对 2D 卡片场景，WebGL2 已经足够（**推断**），WebGPU 不作为入围条件。

## 外壳：浏览器、Tauri 还是 Electron

| 外壳 | 渲染引擎 | 优点 | 代价 / 风险 |
|---|---|---|---|
| Windows 上的 Edge/Chrome | Chromium | 零安装；Windows 原生 IME（TSF）和剪贴板；GPU 直通，不经过 WSLg | 标签页不是 companion 能拥有的进程：`:quit-for-restart`、recovery、ownership 契约都要重新设计；浏览器沙箱读不到 WSL 文件，必须有桥接服务 |
| Tauri 2（`@tauri-apps/api` 2.12.4） | Windows 上是 WebView2（Chromium，可自更新，Win11 预装），Linux 上是 WebKitGTK（[Webview Versions](https://v2.tauri.app/reference/webview-versions/)） | 单一进程，可以做原生窗口 | Windows 版必须在 Windows 构建和安装，推翻 ADR 0007 的“无 Windows 原生路径”（map 允许部分推翻）。在 WSLg 里跑 Linux 版等于 WebKitGTK 加 WSLg 呈现路径，解决不了现有问题 |
| Electron（45.0.0） | 自带 Chromium | 渲染行为与 Chrome 一致，可控 | 体积大；同样是 Windows 原生分发 |

WSLg 呈现路径的一手说明（[WSLg README](https://github.com/microsoft/wslg)）：首版 vGPU
通过系统内存与 Weston 交互，独显上渲染结果要先从显存拷到内存，再在 Windows 侧上传 GPU，
开销与呈现帧率成正比，极高帧率时可达 50%。所以“前端也留在 WSLg”不在推荐范围内。

**推断**：原型阶段用浏览器即可，渲染路径与 WebView2/Electron 都是 Chromium；外壳选择
推迟到 ADR，由进程身份契约决定。

## 跨 Windows/WSL 的文件与通信代价

### 不要让 Windows 侧监听 WSL 文件

- Microsoft 官方建议不要跨操作系统处理文件，Linux 工具的文件应放在 Linux 文件系统
  （[Working across file systems](https://learn.microsoft.com/en-us/windows/wsl/filesystems)）。
  `\\wsl$` 由 Linux 侧的 9P 文件服务器提供
  （[Troubleshooting](https://learn.microsoft.com/en-us/windows/wsl/troubleshooting)）。
- Windows 侧监听 `\\wsl$`：`FindFirstChangeNotification` 直接失败，.NET
  `FileSystemWatcher` 接受这个路径但从不触发
  （[microsoft/WSL#4581](https://github.com/microsoft/WSL/issues/4581)）。该 issue 于
  2024-02-01 被机器人以“一年无活动”自动关闭，**并非修复**。
- 反方向（Windows 改 `/mnt/c` 文件，Linux 收不到 inotify）至今仍未解决
  （[microsoft/WSL#4739](https://github.com/microsoft/WSL/issues/4739)，open，2019-12-06 创建）。
- 结论：如果 Windows 前端直接读写或监听 `.uml-viewer/` 下的 EDN、mailbox 和快照，只能退回
  轮询 9P，每次轮询都是一次跨 VM 的往返。现有“mtime 变化即重载”的语义可以靠轮询维持，
  但延迟和 CPU 占用都是持续成本，空闲 CPU 目标会受影响（**推断**）。

### 推荐：WSL 内桥接，localhost 推送

- 一个小进程（companion 的一部分或旁车）留在 WSL，用 inotify 监听 ext4 上的文件，
  把 IR（EDN 原文，或预先转好的 JSON）和质量快照通过 HTTP 提供给前端，变化通过
  WebSocket/SSE 推送。前端发出的邮件（`:omit`、`:refresh-*`、`:context`）由桥接进程
  写进现有的 outbox 文件，所以 **Herdr 邮件 FIFO、质量门控和 EDN 契约都保持不变**，文件
  语义全部留在 WSL。
- 网络：NAT 模式下 `localhostForwarding` 默认开启，WSL 中绑定通配地址或 localhost 的端口
  可以从 Windows 的 `localhost:port` 访问；mirrored 模式（Win11 22H2+）下双方直接共用
  `127.0.0.1`，此时 `localhostForwarding` 被忽略
  （[Networking](https://learn.microsoft.com/en-us/windows/wsl/networking)，
  [wsl-config](https://learn.microsoft.com/en-us/windows/wsl/wsl-config)）。
- 代价的量级：跨边界的流量只在“数据变化”和“发邮件”时产生，**不在帧循环里**，所以对
  fps 和空闲 CPU 没有结构性影响（**推断**）。没有找到 Microsoft 官方的 localhost 转发
  延迟数据；本次没有实测，因为测量需要在 Windows 侧运行程序，而本票限定不碰 `/mnt/c`。
  原型应记录“文件写入 → 前端重绘完成”的端到端延迟。
- 安全（**推断**）：本机任何进程和网页都能访问 localhost 服务，所以桥接必须只绑定
  `127.0.0.1`、使用一次性 token、校验 `Origin`/`Host`（防 DNS rebinding），否则网页可以
  伪造邮件触发质量工具。

## CJK、IME、剪贴板

- 浏览器原生文字栈处理中文排版和字体回退；IME 组合输入、剪贴板走 Windows 原生路径，
  不依赖 WSLg 的宿主 IME 会话（106 验收里这一项仍“待用户确认”）。这是 Web 前端相对现状
  的主要体验收益（**推断**，原型需要人工确认）。
- 风险集中在 **GPU 文字路径**，而不是输入：PixiJS BitmapText 官方不建议用于 CJK，
  Sigma 4 的 SDF 图集有字形缺失 bug（#1552）。Cytoscape 走 Canvas 文字，没有这个风险。

## 原型建议（交给后续票）

- 同一个合成场景（500 和 2000 类，沿用 `scripts/benchmark_viewer.lua` 的相位划分：
  idle / interaction / detail），在 Windows Edge 中运行，记录 `requestAnimationFrame`
  帧间隔的 p95，以及空闲 30 s 的进程 CPU。
- PixiJS 原型：一个 world Render Group，卡片栅格化为纹理，边合成一个 Graphics/Mesh，
  按需渲染，在缩放阈值处重栅格化，单独统计这些帧。
- Cytoscape 原型：`renderer: { name: 'canvas', webgl: true }` 预览模式，边用直线或 bezier，
  记录 sprite sheet 生成时间。
- 两个原型共用同一个 WSL 桥接（inotify + WebSocket），并测端到端重载延迟。

## 主要风险

1. 没有任何一手资料覆盖“2000 张中文多行卡片”的实测，达标与否要靠原型。
2. 文字随缩放重栅格化的帧尖峰会打到 p95（PixiJS）；sprite sheet 的清晰度和显存占用
   （Cytoscape）。
3. Cytoscape WebGL 不支持正交边和空心箭头，UML 视觉语义需要让步或叠加绘制。
4. 前端离开 WSL 后，Herdr companion 的 ownership、recovery 和 `:quit-for-restart` 契约要
   重新定义“viewer 进程”；浏览器标签页无法被 companion 拥有。
5. localhost 服务带来新的本机攻击面（token + Origin 校验）。
