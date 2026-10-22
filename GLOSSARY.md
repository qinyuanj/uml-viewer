# UML viewer

单上下文术语表，覆盖 viewer、companion 与质量分发领域词汇。

## Language

**Herdr**:
交付平台 WSL2/WSLg 上唯一的 companion 启动后端。
不支持 Windows 原生运行；macOS 当前不承诺。

**selected Herdr**:
本次启动时 PATH 上所有兼容 Herdr 中版本最高的那一个；同一次启动内所有 Herdr 调用都用它。
_Avoid_: default herdr, PATH herdr

**companion**:
由 viewer 管理的外部 Claude 会话，按序消费邮件队列、执行质量命令并驱动图展示。

**quality dispatch**:
viewer 对 CRAP、mutation 与重复检测请求的通用分发：按项目配置调用标准工具，失败时停止后续 IR 与展示。

**Herdr migration**:
将项目从旧 companion 后端迁到 Herdr 的过程：先由旧版本正常结束并归档旧 ownership，保留会话、policy/proposal 与邮件队列，再 fresh 创建 Herdr companion。不得跨后端使用 `--restart`。
_Avoid_: reindexing, in-place upgrade

**源码图**:
按源码命名空间嵌套得到的图，不含任何提案分组。
_Avoid_: 真实图, real diagram（界面文案中）

**提案**:
policy 中一个具名分组，不存在于源码里；与源码图并列。
_Avoid_: 方案, proposal（界面文案中）

**场景核心**:
viewer 中把 IR 与质量快照变成可绘制场景的那部分：质量叠加、源码图/提案视图、declutter、布局、路由，以及导航、命中测试与提案写回。不含绘制、窗口与输入。
_Avoid_: 引擎, 后端, scene service

**隐藏**:
把某个元素从当前提案或真实图中排除的操作。
_Avoid_: 省略, omit（界面文案中）

## Lua 版本

**分析语言版本**:
质量工具理解被分析 Lua 源码时采用的版本，即 Lua 5.4；crapper 与 mutator 只在该版本下产出覆盖率。
_Avoid_: 运行时版本, Lua 5.1

**viewer 运行时**:
本仓 viewer 自身运行所用的解释器，即 LOVE 内嵌的 LuaJIT（Lua 5.1 语义）。
_Avoid_: Lua 5.4, 主解释器

**纯逻辑模块**:
必须同时满足分析语言版本与 viewer 运行时的模块，只用两者公共子集；CI 在两种解释器下各跑一遍。

**FFI 适配层**:
只在 viewer 运行时中工作的模块，依赖 LuaJIT FFI；在分析语言版本下没有质量数据，按规则显示红色。
_Avoid_: 适配器, adapter
