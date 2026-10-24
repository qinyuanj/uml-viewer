# Herdr startup workspace 对照记录

## 现象

在默认 Herdr 会话的 pane 里运行 `./uml`，companion 的隔离 session 里除了
companion 自己的 workspace，还会多一个标签为 `~` 的空 shell workspace（`w1`），
companion 拿到的是 `w2`。

## 原因

`./uml` 从 pane 继承了 `HERDR_STARTUP_CWD`、`HERDR_PANE_ID`、
`HERDR_SOCKET_PATH` 等 `HERDR_*` 变量，`herdr --session <s> server` 也随之继承。
Herdr 0.9.5 的 headless bootstrap 只要看到 `HERDR_STARTUP_CWD`，就会在这个目录
建一个 startup workspace。出问题的那次会话，server 日志里有
`herdr::server::headless::bootstrap: created startup workspace cwd=/home/<user>`，
时间早于 companion 的 `workspace create`。

attach 客户端不会建 workspace。companion 所有 CLI 调用都显式带 `--session`
和 pane/workspace id，即使继承的 `HERDR_*` 指向另一个 session，调用也只落在所选
session 上；隔离 server 给自己的 pane 重新设置 `HERDR_*`。所以只需要处理
server 的启动环境。

## 对照

`probe.sh` 起两个一次性隔离 session，一个继承 `HERDR_STARTUP_CWD`，一个去掉，
其他条件相同。`probe.txt` 是 2026-10-24 在 WSL2 + Ubuntu 24.04 上用
Herdr 0.9.5 的实际输出（`$HOME` 已替换为 `~`）：继承时出现 `~` workspace，
去掉后启动时没有 workspace。

## 修复

companion 启动隔离 server 前，去掉继承来的 `HERDR_*` 变量，只保留用户自己的
`HERDR_CONFIG_PATH`（`herdr_domain::inherited_pane_env`）。回归测试：
`create_starts_the_server_without_the_inherited_herdr_pane_context`，
`spawn_detached_removes_only_the_named_variables_from_the_child_environment`。
没有用修复后的二进制重跑公共 `./uml` 全流程。
