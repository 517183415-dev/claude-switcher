#!/bin/bash
# Claude 保镖 (Linux) —— 代理离线时自动关闭 claude CLI 进程
# 说明: Linux 上 Claude 使用形态是 网页版(浏览器) + Claude Code CLI。
# 保镖只终止 claude CLI(掉了不会误伤你正在用的浏览器)。
LOG="$HOME/.claude-switcher/bodyguard.log"

while true; do
    if ! pgrep -f 'verge-mihomo|mihomo' >/dev/null 2>&1 || ! ip addr 2>/dev/null | grep -q 'inet 198\.18\.'; then
        if pgrep -x claude >/dev/null 2>&1; then
            pkill -x claude
            echo "$(date '+%Y-%m-%d %H:%M:%S') 代理离线(内核或TUN丢失),已自动关闭 claude CLI" >>"$LOG"
        fi
    fi
    sleep 10
done
