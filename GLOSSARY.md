# UML viewer

单上下文术语表，覆盖 viewer、companion 与质量分发领域词汇。

## Language

**Herdr**:
所有交付平台上唯一的 companion 启动后端：WSL2/WSLg 与 macOS Apple Silicon。
不支持 Windows 原生运行。

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

**隐藏**:
把某个元素从当前提案或真实图中排除的操作。
_Avoid_: 省略, omit（界面文案中）
