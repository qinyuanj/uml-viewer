# UML viewer

单上下文术语表，覆盖 viewer、companion 与质量分发领域词汇。

## Language

**Herdr**:
交付平台（WSL2/WSLg 与 macOS）上唯一的 companion 启动后端。
不支持 Windows 原生运行。

**selected Herdr**:
本次启动时 PATH 上所有兼容 Herdr 中版本最高的那一个；同一次启动内所有 Herdr 调用都用它。
_Avoid_: default herdr, PATH herdr

**companion**:
由 viewer 管理的外部 Claude 会话，按序消费邮件队列、执行质量命令并驱动图展示。

**quality dispatch**:
viewer 对 CRAP、mutation 与重复检测请求的通用分发：按项目配置调用标准工具，失败时停止后续 IR 与展示。

**业务范围**:
三个质量工具共享的源码集合，由项目质量配置声明；范围之外只有测试代码。难以测试的业务代码仍在范围内；缺数据的类在图上照常显示为红。
_Avoid_: mutation 范围, scope（单独使用时）

**Herdr migration**:
将项目从旧 companion 后端迁到 Herdr 的过程：先由旧版本正常结束并归档旧 ownership，保留会话、policy/proposal 与邮件队列，再 fresh 创建 Herdr companion。不得跨后端使用 `--restart`。
_Avoid_: reindexing, in-place upgrade

**源码图**:
按源码命名空间嵌套得到的图，不含任何提案分组。
_Avoid_: 真实图, real diagram（界面文案中）

**提案**:
policy 中一个具名分组，不存在于源码里；与源码图并列。
_Avoid_: 方案, proposal（界面文案中）

**外来包**:
policy 的 `:foreign` 列出的项目外的库，画成椭圆，并用箭头与依赖它的类相连；未列出的外部依赖不上图。项目内、当前视图之外的代码不是外来包，它在类框上显示为药丸。
_Avoid_: 外部节点, external

**类卡片**:
双击类打开的浮动卡片，显示该类的成员、关系与所属包；其邻域由场景核心给出，文案与排版由前端负责。

**源码窗口**:
从类卡片成员打开的浮动窗口，显示该成员所在源文件并定位到其声明行。与**源码图**无关。
_Avoid_: 源码仓, 源码面板, source window（界面文案中）

**定位行**:
源码窗口中，当前打开成员的声明所在的那一行。
_Avoid_: 源码行, 目标行

**依赖链**:
选中一个类之后，它直接或间接依赖的全部类，以及连着这些类的依赖边。沿「这个类依赖谁」往下走，直到没有进一步的依赖；环回到已经走过的类即停。
_Avoid_: 邻接, 一跳

**场景核心**:
viewer 中把 IR 与质量快照变成可绘制场景的那部分：质量叠加、源码图/提案视图、declutter、布局、路由，以及导航、命中测试与提案写回。不含绘制、窗口与输入。
_Avoid_: 引擎, 后端, scene service

**viewer 会话**:
viewer 中不依赖窗口的部分：持有视图状态，应用前端动作、热重载、邮件消费以及正常关闭与重启退出，返回重绘与关闭请求。它读写项目文件，因此**不是**纯逻辑模块；唯一的宿主 seam 是 companion 链接（轮询、停止并指定是否保留、唤醒、是否已 settled）。与「会话快照」（重启时保存的视图）和 Herdr 会话不同。
_Avoid_: app, session state

**正常关闭**:
关窗、终止信号或测试时长结束时 viewer 的退出：以**不保留** companion 的方式停止 helper，helper settled 或 5 秒截止时以 0 退出；不写会话快照。
_Avoid_: quit, 关闭（单独使用时）

**重启退出**:
收到 `:quit-for-restart` 邮件后的退出：以**保留** companion 的方式停止 helper，等到它 settled 后才写会话快照并以 0 退出，供 `./uml --restart` 恢复。
_Avoid_: restart（指退出这一步时）

**隐藏**:
把某个元素从当前提案或真实图中排除的操作。
_Avoid_: 省略, omit（界面文案中）

## 运行时

**viewer 运行时**:
本仓唯一的实现，一个 Rust 二进制（egui/eframe，WSLg + D3D12）；同时承担 viewer、IR 生成、四个扫描器与 companion 的无头角色。
_Avoid_: LOVE, LuaJIT, 双解释器, 主解释器

**分析语言版本**:
质量工具理解被分析源码时采用的语言与版本。本仓自举时是 Rust；Lua 项目仍可被扫描，但仓库不再包含 Lua 运行时。
_Avoid_: 运行时版本, Lua 5.4 分析契约

**纯逻辑模块**:
不依赖窗口与输入、只消费 IR 与质量快照的模块（场景核心、EDN、IR、companion）。只有 FFI/POSIX 适配层与前端依赖宿主能力。
_Avoid_: 适配器, adapter
