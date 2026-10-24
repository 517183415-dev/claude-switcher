#!/bin/bash
# 时区看门狗 (参考模板) —— 安装程序会把 __TZ_TARGET__ 和 __HOME__ 替换后安装
# 作用: 系统时区偏离目标值时自动改回,并记录日志
TARGET="__TZ_TARGET__"

current=$(readlink /etc/localtime 2>/dev/null)
if [ "$current" != "/var/db/timezone/zoneinfo/$TARGET" ]; then
    if /usr/sbin/systemsetup -settimezone "$TARGET"; then
        logger -t local.tz-watchdog "时区异常: [$current] 已恢复为 $TARGET"
    else
        logger -t local.tz-watchdog "时区恢复失败: [$current] 目标为 $TARGET"
        exit 1
    fi
fi
