#!/bin/bash
# Claude 保镖 (参考模板) —— 安装程序会把 __HOME__ 替换为实际用户目录后安装
# 作用: 代理(内核或TUN)离线时,自动关闭 Claude,杜绝裸连重试
LOG="__HOME__/Library/Logs/claude-bodyguard.log"

ok=1
pgrep -f verge-mihomo >/dev/null 2>&1 || ok=0
ifconfig 2>/dev/null | grep -q 'inet 198\.18\.' || ok=0

if [ "$ok" -eq 0 ]; then
    if pgrep -f '/Applications/Claude.app/Contents/MacOS/Claude' >/dev/null 2>&1; then
        pkill -f '/Applications/Claude.app/Contents/MacOS/Claude'
        echo "$(date '+%Y-%m-%d %H:%M:%S') 代理离线(内核或TUN丢失),已自动关闭 Claude" >>"$LOG"
    fi
fi
