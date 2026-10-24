#!/bin/sh
# 一次性隔离 session 对照：继承 HERDR_STARTUP_CWD 与否，headless server
# 启动后的 workspace。结束时停止并删除 probe session。
set -u
for mode in inherited removed; do
  s=probe-startup-$mode-$$
  if [ $mode = inherited ]; then
    setsid env HERDR_STARTUP_CWD="$HOME" herdr --session "$s" server </dev/null >/dev/null 2>&1 &
  else
    setsid env -u HERDR_STARTUP_CWD herdr --session "$s" server </dev/null >/dev/null 2>&1 &
  fi
  sleep 4
  echo "== $mode"
  herdr --session "$s" workspace list
  grep -o 'created startup workspace.*' "${XDG_CONFIG_HOME:-$HOME/.config}/herdr/sessions/$s/herdr-server.log" || echo '(no startup workspace logged)'
  herdr session stop "$s" >/dev/null
  herdr session delete "$s" >/dev/null
done
