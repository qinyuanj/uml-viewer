# 0004 Herdr executable selection: highest compatible on PATH

**Status**: accepted

PATH 上存在多个 `herdr` 时，不按惯例取第一个，而是在所有通过 ADR 0003 门槛（纯数字 `X.Y.Z` 且 `>= 0.9.3`、schema 为 protocol 22）的候选中选版本最高者；同版本取 PATH 靠前者。原先预检、server 启动与后续 CLI 调用各自按 PATH 解析，可能不是同一个 binary，预检因此失去意义；现在一次启动内所有调用都使用选中的同一绝对路径。

最新版不兼容时退到本机已装的最高兼容版本，而不是报错停止：用户升级 Herdr 后不应再次卡死（ADR 0003 的同一问题）。"No automatic download, upgrade or fallback" 仍指不下载、不升级，不禁止在已装版本中挑选。无兼容候选时报 guidance 并列出每个候选的路径与拒绝原因。

候选只来自 PATH，不扫描 `~/.herdr/packages/...` 等安装目录：PATH 是用户声明的位置，猜测安装布局易错。`--restart` 按同一规则重选，与运行中 server 的兼容性由 endpoint protocol 检查保证，不把 binary 身份当作 ownership 的一部分。选择规则与平台无关；交付平台见 ADR 0007（spec #106 起仅 WSL2 + WSLg，PATH 按 `:` 分隔）。
