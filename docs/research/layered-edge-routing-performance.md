# 分层图边路由与曲线展平的性能：业界做法与本仓对照（#186）

调研日期 2026-10-28。范围：#186 中 `4bed821` 把 level 拆成子层之后，2000 类基准编译变慢的问题。具体涉及三件事：
三次贝塞尔曲线的展平精度与点数，路由后折线的化简，大图边路由在编译端和渲染端之间怎样分工；最后是布局基准怎么设计。
分层、虚拟节点、Brandes–Köpf 拉直已在 [`layered-long-edge-layout.md`](layered-long-edge-layout.md) 中讨论过，本文不重复。
源码引用指 2026-10-28 抓取的各项目默认分支（下表列出 commit），行号可能随上游变化。只引用一手来源（论文、官方文档、源码）；
没有拿到一手依据的地方标 **未核实**；本仓实测和由此得出的推断单独注明。

| 项目 | 默认分支 | 抓取 commit（提交日期） |
| --- | --- | --- |
| kurbo | main | 9366cda（2026-10-08） |
| lyon | main | 3c1f005（2026-10-23） |
| tiny-skia | main | 5d47547（2026-08-20） |
| Skia | main | 5d443a6（2026-10-28） |
| Cairo | master | 7475596（2026-09-22） |
| egui（epaint） | main | fc9d66d（2026-10-26） |
| Graphviz | main | 8cc0e9a（2026-10-28） |
| ELK | master | b715fb1（2026-10-26） |
| Adaptagrams（libavoid） | master | 840ebcf（2025-10-29） |
| dagre | master | 877ead2（2026-10-24） |
| OGDF | master | 1a40505（2026-04-21） |

## 结论摘要

1. **本仓的点数大头在展平，不在路由。** 实测 2000 类场景（§2.2）：路由点 5.66 万，经 `round_corners` 后 9.9 万
   （与 #186 的 “9.8 万” 吻合），对应 8.6 万段三次曲线；固定 16 步展平后约 **139 万**个采样点。路由点里
   共线的内点只有 30 个，因为 `route.rs` 的 `collapse` 已经合并了轴向共线点。所以 #186 的方向 (1)
   “合并共线车道点” 在这张图上几乎没有收益，方向 (2) 才是主要杠杆。
2. **业界展平一律按距离容差，不用固定参数步长。** 默认容差：kurbo 建议 0.25，Skia GPU 为 0.25 设备像素，
   lyon、Cairo、egui 都是 0.1。点数大约与 √(1/容差) 成正比（kurbo 文档原话）。本仓的固定 16 步两头都不好：
   中位误差只有 0.02 px（过采样），最长的曲线段误差却达 11.3 px（欠采样）。几何自适应（递归细分，
   或 Levien 的抛物线积分法）在容差 0.25 时，点数可降到现在的约 47–68%。**Wang 公式在本仓反而更差**
   （0.25 时 1.74 倍）：它按均匀参数取点，而本仓 B 样条在长直段上的参数化很不均匀。
3. **成熟系统输出的是曲线（控制点），展平交给渲染端，并按缩放选容差。** Graphviz `Proutespline` 输出
   贝塞尔控制点；ELK 的 `FinalSplineBendpointsCalculator` 输出贝塞尔控制点；dagre 只输出折点，由渲染端
   （d3 `curveBasis`，与本仓 `basis_path` 的公式逐项相同）插值。lyon 文档写明容差 “通常随缩放级别选取”。
   本仓在编译期按世界坐标展平，而 viewer 最大可放大到 10 倍：编译期无论选什么世界容差，屏幕误差都会被放大 10 倍。
4. **“几何不变” 的硬约束下，展平手段全都会改几何。** route goldens 以 1e-6 比对 `:strokes`
   （`crates/scene/tests/route_goldens.rs:1-6, 155-166`）。只要改展平，`:points` 和 `:curve` 可以不变，
   但 `:strokes` 必然变，goldens 要重录。在几何严格不变的前提下，能做的只有实现层面的优化：按点分配的
   EDN `Value` 改成 `[f64; 2]` 数组、按边并行、包围盒提前退出。这几项没有业界出处，是本仓推断。
5. **基准：业界常用 Rome graphs（11,582 张图）、AT&T/North graphs 和随机 DAG（graphdrawing.org 提供 10–100 节点的
   GraphML 子集），还有 DAGmar 分层图生成器。** 这些都是小图，测的是质量，不是规模。对 2000 类这一量级，
   需要像 #186 方向 (3) 那样自己生成按 `:levels` 分层的场景。近线性最好在多个规模上拟合 log–log 斜率，
   不要只看单一比值（推断）。ELK、OGDF 的回归基准具体用了哪些图集 **未核实**。

## 1. 各方案

### 1.1 三次贝塞尔展平与误差界

**kurbo（Raph Levien）**

- `flatten(path, tolerance, callback)`（[kurbo] `kurbo/src/bezpath.rs:622`）。文档：tolerance 是曲线与折线之间的
  Hausdorff 距离上界，“尽量但不绝对保证”；“对抗锯齿渲染，0.25 被确定为效果良好的取值”；“段数大致与容差的
  平方根倒数成正比”（`bezpath.rs:589-596`）。
- 算法：先把三次曲线拆成若干二次曲线（用掉 `TO_QUAD_TOL = 0.1`，即 10% 的误差预算，`bezpath.rs:555, 664-667`），
  然后对每段二次曲线用抛物线弧长积分的闭式近似（`quadbez.rs:57` `estimate_subdiv`，`quadbez.rs:204-211`
  `approx_parabola_integral` / `inv_integral`）估算所需段数，把总段数 n 均匀分配在这个积分量上
  （`bezpath.rs:668-700`）。所以分点不是按参数 t 均匀分布，而是在曲率大的地方更密。原理见 [levien-flatten]
  （源码注释的引用；博文本身本文未抓取，**未核实**）。
- 另有实验性的 `simplify` / `fit_to_bezpath`，用于把曲线拟合成更少的贝塞尔段（`simplify.rs:4-10`，
  `fit.rs:150-167`），文档注明精度 “不是严格保证”。

**lyon**

- `CubicBezierSegment::for_each_flattened` 用 **Wang 公式**算出段数 n，再按参数 t 均匀取点
  （[lyon] `crates/geom/src/cubic_bezier.rs:612-631`，`flattened_segments_wang` 在 `:1418-1450`）：
  n⁴ = max(|p0−2p1+p2|², |p1−2p2+p3|²)·36 / (8·tol)²。
- 容差语义：“曲线与其线性近似之间的最大距离……通常随缩放级别选取”（`crates/geom/src/lib.rs:52-56`）。
  `StrokeOptions::DEFAULT_TOLERANCE` 与 `FillOptions::DEFAULT_TOLERANCE` 都是 0.1
  （`crates/tessellation/src/lib.rs:367, 478`）。
- `num_quadratics` 的注释写明公式 “由 Raph Levien 从 Sederberg 的 CAGD 讲义 §10.6 推导”（`cubic_bezier.rs:393-396`）。

**Skia / tiny-skia**

- Skia Ganesh：`kDefaultTolerance = 0.25`，注释说是 “屏幕空间中线段偏离真实曲线的最大距离……近似光栅路径的
  16 采样精度，即四分之一像素”（[skia] `src/gpu/ganesh/geometry/GrPathUtils.h:27-32`）；每条曲线最多
  `1 << 10` 个点（`:34-35`）。
- Skia 硬件细分：`kPrecision = 4`，即误差不超过 1/4 像素（`src/gpu/tessellate/Tessellation.h:28-29`）。
  段数用 Wang 公式（`WangsFormula.h:26-35`，引 Goldman 2003 §5.6.3），并且是**乘上视图变换之后**算的
  （`PatchWriter.h:509` 传入 `fApproxTransform`）。
- tiny-skia 光栅化：三次曲线按 2 的幂分段，`shift` 由控制点离基线的距离决定，注释 “约 1/8 像素精度”，
  上限 `MAX_COEFF_SHIFT = 6`，即最多 64 段（[tiny-skia] `src/edge.rs:17, 371-386, 505-519`）。
  conic 转二次曲线用 0.25 容差（`path/src/path_geometry.rs:876`）。

**Cairo**

- 默认容差 `CAIRO_GSTATE_TOLERANCE_DEFAULT 0.1`（设备单位）（[cairo] `src/cairoint.h:723`，`cairo-gstate.c:95`）。
- 算法：递归 de Casteljau 二分。误差上界取两个内控制点到弦的距离（`cairo-spline.c:173-200`
  `_cairo_spline_error_squared`），小于 tol² 就停止（`:240-256`，`:267`）。这是最常见的 “控制多边形平坦度” 判据。

**egui / epaint（本仓渲染端）**

- `TessellationOptions::bezier_tolerance` 默认 0.1（[egui] `crates/epaint/src/tessellator.rs:720, 748`）。
- `flatten_cubic_bezier_with_t` 把容差的 20% 用于拆成二次曲线、80% 用于展平（`shapes/bezier_shape.rs:521-530`），
  注释标明移植自 lyon。不传容差时，回退为 |x0−x3|·0.001（`:249-251`）。
- 本仓 viewer 目前不使用 epaint 的曲线形状，而是自己画折线（`crates/viewer/src/paint.rs` 的 `add_segments`）。

**小结：点数与误差。** 均匀参数取点（Wang）的误差界与 1/n² 成正比，因此 n ∝ √(1/tol)
（[skia] `WangsFormula.h:26-35`）；几何自适应（Cairo 的递归二分、kurbo 的抛物线积分）也是同一阶，
差别在常数和点的分布。Wang 的界由二阶差分 |p0−2p1+p2| 决定：控制点沿直线分布但间距不等时，
二阶差分很大，曲线却是直的，这时它会严重高估所需段数。§2.2 的实测正好落在这种情况。

### 1.2 折线化简与冗余拐点

- **共线合并**是路由器的标准后处理：
  - libavoid：`Polygon::simplify()` 把三点共线（`vecDir == 0`）的中间点删掉（[libavoid]
    `cola/libavoid/geomtypes.cpp:532-545`）；对外的 `displayRoute()` 返回的就是简化后的路线
    （`connector.cpp:631, 648-655`）。
  - ELK 正交路由：起止坐标差小于 `TOLERANCE = 1e-3` 的超边段 “是直线，不占 slot，也不需要折点”
    （[elk] `p5edges/orthogonal/OrthogonalRoutingGenerator.java:66, 229-232`），只有 |sourceX−targetX| 超过容差
    才加折点（`direction/NorthToSouthRoutingStrategy.java:62-68`）。
  - ELK 的 `LongEdgeJoiner` 默认**不把** dummy 位置写成折点。只有打开 `unnecessaryBendpoints` 才会写
    （`intermediate/LongEdgeJoiner.java:70, 143-147`）；该选项默认 false，说明是 “默认只在边改变方向处加折点”
    （`Layered.melk:1274-1282`）。
  - dagre 反其道：`undo` 把每个 dummy 坐标都推入 `points`（见前一篇 §1.3），曲线化交给渲染端。
- **Graphviz 样条**：在框序列围成的多边形内先求最短路（`Pshortestpath`，[gv] `lib/common/routespl.c:448`），
  这条最短路本身就是拐点最少的折线。`splines=polyline` 时直接输出它（`:461` `make_polyline`），否则用
  `Proutespline` 拟合（`:478`）。拟合（`lib/pathplan/route.c:93-154` `reallyroutespline`）先用**一段**三次曲线
  拟合整条折线；如果它穿出多边形（`splinefits`，`:206-235`），就在偏差最大的输入点处一分为二，递归拟合。
  所以输出段数由障碍决定，不由折线点数决定。pathplan README 写明 “输出样条不必经过输入路径的所有点”。
- **Ramer–Douglas–Peucker / Visvalingam–Whyatt**：两者是制图学里通用的折线化简算法 [DP73][VW93]。
  RDP 按到弦的距离递归保留点；Visvalingam 按三角形有效面积逐个删点。本文没有在上述图布局工具中找到
  对它们的使用（**未核实**），原论文内容也未抓取（**未核实**）。本仓 viewer 的 `lod::simplify` 是单遍的
  “距弦距离” 贪心，属于同一类判据（`crates/viewer/src/lod.rs:58-83`）。

### 1.3 大图边路由的性能做法

- **Graphviz dot：等价边只路由一次。** 先排序，让 “端点和端口都相同、分开路由也会得到同样路线” 的边相邻
  （[gv] `lib/dotgen/dotsplines.c:329-335`）；每组只算一条样条，其余按 `Multisep` 平移（`:363`，`:1885-1892`）。
  对齐的虚拟链走 `straight_path` 直线段，不进样条拟合（`:1828`，`:2049`）。
- **Graphviz：先框后线。** 每条边先得到一串框，框内求最短路再拟合；拟合完用 `limitBoxes` 按样条采样，
  把框收紧回实际占用的范围，留给后续的边（`routespl.c:232-262`）。采样密度从 `INIT_DELTA 10` 开始，
  最多加倍 `LOOP_TRIES 15` 次（`:271-274, 544-560`）。这里的采样只用于回收空间，不作为输出几何。
- **ELK splines：** `SplineEdgeRouter` 不计算最终折点，只决定层间 slot 和各段（`p5edges/splines/SplineEdgeRouter.java:51-76`）；
  `FinalSplineBendpointsCalculator` 最后把它们变成贝塞尔控制点（`intermediate/FinalSplineBendpointsCalculator.java:47-60`）。
  默认 `SLOPPY` 模式 “用更少的控制点换更弯的边，但可能压到节点”（`Layered.melk:615-626`）。
  ELK 在大图上的实测复杂度与瓶颈 **未核实**。
- **libavoid：** 正交可见图可在 O(n²) 时间内构造；单条连线的 A* 最优路由为 O(n² log n)，n 为障碍数
  [WMS09 定理 1、2]。后续 “Seeing Around Corners” 提出 obstacle-hugging 路线，以减少拓扑等价路线的重复搜索
  [MSW14]。（两处定理均来自论文 PDF 的检索摘录，全文本文未逐页核对。）libavoid 的定位是交互式编辑器的
  增量路由（[libavoid-doc]）。按这个复杂度，2000 节点、6000+ 边整图重路由并不是它的目标场景（推断）。
- **精度分工：** 上面几家的布局输出都是**曲线控制点或折点**；展平发生在渲染端，容差以设备像素计
  （Skia 0.25、Cairo 0.1、lyon/egui 0.1），并按当前变换计算（Skia `fApproxTransform`；lyon 文档说
  “随缩放级别选取”）。也就是说：编译端只负责拓扑与控制点的精度，渲染端负责像素精度。

### 1.4 布局与路由基准

- graphdrawing.org 的数据页列出（[gd-data]）：DAGmar（DAG 与分层图生成器，附预生成的分层图套件）；
  GDToolkit 测试集，即 **Rome graphs**（ALF/CU 10000 张连通无向图、ALF/CD 10000 张连通有向图，以及若干
  RND 集）；**AT&T Graph Catalog**（有向、无向）；还有从中导出的 GraphML 版本，含 North（AT&T 有向图）、
  Rome、随机 DAG，“每个归档都是 10 到 100 个节点的图”。
- Rome graphs 来自 Di Battista 等对四种算法的实验比较，用了 11,582 张图 [DGL+97]（数字取自出版社摘要页的检索摘录）。
  DAG 绘制的实验研究另见 [DGL+00]（内容 **未核实**）。
- 这些图集最多 100 个节点，用于比较交叉数、弯折数、面积等质量指标，不适合衡量 2000 类量级的伸缩性。
  Graphviz、ELK、OGDF 是否用它们做性能回归 **未核实**。
- 合成与真实：合成图便于按规模扫参数并控制形态（DAGmar 就是为此而做的生成器）；真实图反映实际分布。
  #186 方向 (3) 的 “按 `:levels` 分层的合成场景” 属于前者，但要按真实项目的形态（每 level 的类数、跨 level 边比例）
  来定参数（推断）。
- 近线性：本仓只比较 500→2000 一个比值（`crates/scene/tests/bench_scene.rs:88-118`）。更稳的做法是在 3–4 个规模上
  测量，拟合 log(时间)–log(规模) 斜率，并同时记录点数等计数器，让 “数据变多” 与 “算法变慢” 能分开（推断，无一手出处）。

## 2. 与本仓现状对照

### 2.1 现状

- `route()` 产出 `:points`；`collapse` 合并重复点和误差 0.51 内的轴向共线点（`crates/scene/src/route.rs:50-75`）。
  `hits` 对每段折线线性扫描全部节点（`route.rs:337-354`）。
- `basis_path`：先 `round_corners`（半径 24，每个急转弯点变成入、出两个点，`curve.rs:21-45`），再做均匀三次
  B 样条，每个点生成一段三次曲线（`curve.rs:97-136`）。公式与 d3 `curveBasis` 完全相同（[d3-basis]）。
- `flatten_path` 固定参数步长，`compose.rs:195` 传入 0.0625，即每段 16 个采样点（`curve.rs:218-245`）。
- `prepare_scene`：每条边先展平，再用 `gap_polyline` 在类框处断开，结果存为 `:strokes`（EDN `Value`，每点两个数值对象）
  （`compose.rs:182-210`；`geom.rs:179-270`）。
- viewer：Frames 档用 `lod::simplify` 按缩放化简（8 px 最小间距、0.5 px 弦距，`render_cache.rs:82-92`）；
  Full 档直接画全部点。缩放范围 0.1–10（`crates/viewer/src/driver.rs:157`）。

### 2.2 实测（`synthetic-2000.edn`）

方法：`target/release/uml-viewer scene --ir bench/scenes/synthetic-2000.edn --output …`（release 二进制，
构建于 2026-10-28 21:33，晚于 `4df1614` 之前的全部 scene 改动；墙钟 3.03 s，峰值 RSS 276 MB）。从导出的 `:points`
用脚本复现 `round_corners` 与 `basis_path`，统计点数，并估算各种展平方式的段数。该导出走 `cli::project`，
按 section 路由、不裁剪（`bench_scene.rs:138-139`），与 `compile_view` 的计数可能略有差异；#186 说的 “单边最多 60 点” 本文没有复现。

| 量 | 值 |
| --- | --- |
| 边 / 路由点 / 单边最多 | 6,495 / 56,620 / 24 |
| `round_corners` 后点数 / 单边最多 | 99,054 / 46 |
| 三次曲线段 | 86,064（其中三点精确共线的只有 30 段） |
| 路由点中共线的内点 | 30 |
| 固定 16 步展平的采样点 | ≈ 1,390,000 |
| 固定 16 步的弦误差（世界 px） | 中位 0.021，p99 2.42，最大 11.26 |
| 曲线段弦长（世界 px） | 中位 354，p99 12,350，最大 53,555 |

各展平方式所需段数，相对于固定 16 步：

| 容差（世界 px） | Wang（均匀 t，lyon/Skia 式） | 控制多边形递归二分（Cairo 式） | 曲率积分估计（kurbo 式的理想值） |
| --- | --- | --- | --- |
| 0.1 | 2.73× | 1.01× | 0.73× |
| 0.25 | 1.74× | 0.68× | 0.47× |
| 0.5 | 1.24× | 0.49× | 0.35× |

曲率积分一列用 ∫√(κ/8tol) ds 近似最优段数，是估计值，不是运行 kurbo 得到的结果。

解读：
- “9.8 万” 对应的是 `round_corners` 之后的点数，并不是路由点。每个点在 `prepare_scene` 中再放大约 14 倍。
  这一点可以由计数核对；但耗时是否主要花在采样点上，本文没有做 profile（**未核实**）。
- 共线合并（方向 1）在这张图上只能去掉 30 个点：`collapse` 已经做过了。而且删掉一个路由点会改变
  B 样条的形状（每段曲线由相邻三个点决定），所以它也不是 “几何不变” 的。
- 固定步长在长段上欠采样（11 px），在短段上过采样。几何自适应在 0.25 的容差下点数约减半，同时误差有上界；
  Wang 公式因为长直段的参数化不均匀，反而更多。

### 2.3 各手段与 #186 约束

| 编号 | 手段 | 业界依据 | 改哪里 | `:points` / `:curve` | `:strokes`（goldens） |
| --- | --- | --- | --- | --- | --- |
| P1 | 存储改用 `[f64; 2]`，不再为每个点建 EDN `Value`；`:strokes` 只在导出时序列化 | 无（实现层面） | `compose.rs:182-210`，以及 viewer 读取 strokes 的路径 | 不变 | 数值不变；如果 scene 契约改变，导出时要等价 |
| P2 | 按边并行 `prepare_scene`，保持输出顺序 | 无 | 同上 | 不变 | 不变 |
| P3 | `gap_polyline` 先拿整条折线的包围盒和 `rects` 比较，不相交就整条保留 | 无（`gap_polyline` 已在 `rects` 为空时短路，`geom.rs:259-261`） | `geom.rs` | 不变 | 不变 |
| A1 | 固定 16 步改为几何自适应（Cairo 式递归二分，或 kurbo 式），世界容差约 0.25 | Cairo、kurbo、Skia | `curve.rs:218`，`compose.rs:195` | 不变 | **变**，goldens 要重录 |
| A2 | 编译端只输出 `:curve`（控制点），裁剪改成在曲线参数上做，展平移到 viewer 并按缩放选容差 | Graphviz、ELK、dagre/d3、lyon 文档、Skia | scene 契约，`prepare_scene`，`hit.rs`，viewer 渲染 | 不变 | **去掉或改义**，契约变化，需要 ADR 层面的讨论 |
| C1 | 路由后合并共线点 | libavoid `simplify`，ELK `TOLERANCE` | `route.rs` | 本图几乎无效；删点会改变 B 样条 | 变 |
| C2 | 等价边只路由一次（同端点、同端口） | dot `edgecmp` / `Multisep` | `route.rs` | 只在存在等价边时有效；本仓的 `ft/tt` 散布让同端点的边端口不同，适用性 **未核实** | 变 |

关于 A2：它从结构上解决了缩放下的精度问题——Full 档放大到 10 倍时，编译期用世界坐标展平的 11 px 误差会变成 110 px。
但 `:strokes` 是对外的场景输出，route goldens、`hit.rs:142-150` 的命中测试和 viewer 都依赖它。
ADR 0008 只要求 “layout/route 严格通过 goldens……几何输出不能变”，没有规定必须在编译期展平。所以 A2 不违反 ADR，
但需要先决定 goldens 比对 `:curve`，不再比对 `:strokes`。

## 3. 推荐顺序

1. **先测再改：** 给 `prepare_scene` 加上计数器（采样点数、`gap_segment` 调用次数）和分段计时，确认 1.81 s 中有多少是
   EDN 分配、多少是裁剪。这个结论本文未核实，P1 和 P3 的优先级取决于它。
2. **P1 + P3（+ P2）：** 在几何严格不变的约束内，只剩这几项可做；它们不改 goldens，可以和 #185 解耦。
3. **A1，随 #185 一起改：** #185 本来就要改几何，可以和它一起重录 goldens。容差取 0.25 世界 px（kurbo/Skia 的值），
   预期采样点降到约一半，并且消除 11 px 的欠采样。不要用均匀参数的 Wang 公式（见 §2.2）。
4. **A2 作为长期方向：** 和业界一样，编译端只给控制点，viewer 按缩放展平。需要先讨论 scene 契约和 goldens 的比对对象。
5. **基准：** 按 #186 方向 (3) 增加一个按 `:levels` 分层的合成场景，在 3–4 个规模上测量并拟合斜率，同时记录点数。
   C1 在本仓图上收益可以忽略，不建议单独做。

## 参考文献

- [kurbo] linebender/kurbo `kurbo/src/bezpath.rs`、`quadbez.rs`、`simplify.rs`、`fit.rs`。https://github.com/linebender/kurbo （本仓 `Cargo.lock` 中也有 kurbo 0.13.1，是间接依赖）
- [levien-flatten] R. Levien, *Flattening quadratic Béziers*, 2019. https://raphlinus.github.io/graphics/curves/2019/12/23/flatten-quadbez.html （由 kurbo 源码引用，本文未抓取）
- [lyon] nical/lyon `crates/geom/src/cubic_bezier.rs`、`crates/geom/src/lib.rs`、`crates/tessellation/src/lib.rs`。https://github.com/nical/lyon
- [skia] Skia `src/gpu/ganesh/geometry/GrPathUtils.h`、`src/gpu/tessellate/{Tessellation.h,WangsFormula.h,PatchWriter.h}`。https://skia.googlesource.com/skia
- [tiny-skia] linebender/tiny-skia `src/edge.rs`、`path/src/path_geometry.rs`。https://github.com/linebender/tiny-skia
- [cairo] Cairo `src/cairoint.h`、`src/cairo-gstate.c`、`src/cairo-spline.c`。https://gitlab.freedesktop.org/cairo/cairo
- [egui] emilk/egui `crates/epaint/src/tessellator.rs`、`crates/epaint/src/shapes/bezier_shape.rs`。https://github.com/emilk/egui
- [gv] Graphviz `lib/common/routespl.c`、`lib/pathplan/{route.c,README}`、`lib/dotgen/dotsplines.c`。https://gitlab.com/graphviz/graphviz
- [elk] ELK `plugins/org.eclipse.elk.alg.layered/src/org/eclipse/elk/alg/layered/`（`p5edges/orthogonal/`、`p5edges/splines/SplineEdgeRouter.java`、`intermediate/{LongEdgeJoiner,FinalSplineBendpointsCalculator}.java`、`Layered.melk`）。https://github.com/eclipse-elk/elk
- [libavoid] mjwybrow/adaptagrams `cola/libavoid/geomtypes.cpp`、`connector.cpp`。https://github.com/mjwybrow/adaptagrams
- [libavoid-doc] https://www.adaptagrams.org/documentation/libavoid.html
- [dagre] dagrejs/dagre `lib/layout.ts`（`points` 的生成在 590-622 行）。https://github.com/dagrejs/dagre
- [d3-basis] d3/d3-shape `src/curve/basis.js`。https://github.com/d3/d3-shape/blob/main/src/curve/basis.js
- [WMS09] Wybrow, Marriott, Stuckey. *Orthogonal Connector Routing*. GD 2009, LNCS 5849, 219–231. https://users.monash.edu/~mwybrow/papers/wybrow-gd-2009.pdf
- [MSW14] Marriott, Stuckey, Wybrow. *Seeing Around Corners: Fast Orthogonal Connector Routing*. Diagrams 2014, LNCS 8578, 31–37. https://users.monash.edu/~mwybrow/papers/marriott-diagrams-2014.pdf
- [gd-data] Graph Drawing 数据集页面. https://graphdrawing.unipg.it/data.html （原 http://www.graphdrawing.org/data.html 重定向至此）
- [DGL+97] Di Battista, Garg, Liotta, Tamassia, Tassinari, Vargiu. *An experimental comparison of four graph drawing algorithms*. Comput. Geom. 7(5–6):303–325, 1997. https://www.sciencedirect.com/science/article/pii/S0925772196000053
- [DGL+00] Di Battista, Garg, Liotta, Parise, Tamassia, Tassinari, Vargiu, Vismara. *Drawing Directed Acyclic Graphs: An Experimental Study*. IJCGA 10(6):623–648, 2000.（未抓取）
- [DP73] Douglas, Peucker. *Algorithms for the reduction of the number of points required to represent a digitized line or its caricature*. Cartographica 10(2), 1973.（未抓取）
- [VW93] Visvalingam, Whyatt. *Line generalisation by repeated elimination of points*. Cartographic Journal 30(1), 1993.（未抓取）
