#!/bin/bash
# Claude 账号切换器 - 核心脚本 (通用版)
# 用法: claude-switch.sh check | status | switch A|B | checkdetail | fix | set-region
set -u
export LC_ALL=en_US.UTF-8

# ============ 安装时可配置项(安装程序会自动改写) ============
PROXY_MODE="clash-verge"       # 代理客户端: clash-verge|shadowrocket|sing-box|clashx|surge|auto
PROXY_GROUP="IpEqual"          # Clash 代理组名(安装时自动探测)
COUNTRY_TARGET="US"            # 目标出口国家(安装时按出口自动识别)
TZ_TARGET="America/New_York"   # 锁定时区,应与出口IP所在城市一致
LOCALE_TARGET="en_US"          # 锁定地区格式
# ============================================================

DATA="$HOME/Library/Application Support/Claude"
BASE="$HOME/.claude-switcher"
LOG="$HOME/Library/Logs/claude-switch.log"
CLASH_DIR="$HOME/Library/Application Support/io.github.clash-verge-rev.clash-verge-rev"
ROUTE_MISSED=""
mkdir -p "$BASE/accounts"

log() { echo "$(date '+%Y-%m-%d %H:%M:%S') $1" >>"$LOG"; }

# TUN 在线判定:存在 fake-ip 网段地址即视为 TUN 已接管(兼容各种 TUN 设备名)
tun_up() { ifconfig 2>/dev/null | grep -q 'inet 198\.18\.'; }

# 代理客户端显示名(供检查细节输出)
proxy_name() {
    case "$PROXY_MODE" in
        clash-verge)  echo "Clash Verge" ;;
        shadowrocket) echo "Shadowrocket" ;;
        sing-box)     echo "sing-box" ;;
        clashx)       echo "ClashX" ;;
        surge)        echo "Surge" ;;
        auto)         echo "代理客户端" ;;
        *)            echo "$PROXY_MODE" ;;
    esac
}

# 可 open -a 启动的客户端 App 名(sing-box/auto 无标准 App,返回空)
proxy_app_name() {
    case "$PROXY_MODE" in
        clash-verge)  echo "Clash Verge" ;;
        shadowrocket) echo "Shadowrocket" ;;
        clashx)       echo "ClashX" ;;
        surge)        echo "Surge" ;;
        *)            echo "" ;;
    esac
}

# Shadowrocket 在 Mac 上表现为 VPN 配置;launchd/非图形环境下 scutil 可能查不到该行,此时以 TUN 判定为准(避免误杀)
shadowrocket_up() {
    nc=$(scutil --nc list 2>/dev/null)
    if echo "$nc" | grep -q 'com.liguangming.Shadowrocket'; then
        echo "$nc" | grep 'com.liguangming.Shadowrocket' | grep -q '(Connected)'
    else
        tun_up
    fi
}

# 代理内核在线判定:按安装时识别的客户端分派;auto 时任一命中即活
proxy_alive() {
    case "$PROXY_MODE" in
        clash-verge)  pgrep -f verge-mihomo >/dev/null 2>&1 ;;
        shadowrocket) shadowrocket_up ;;
        sing-box)     pgrep -f sing-box >/dev/null 2>&1 ;;
        clashx)       pgrep -x ClashX >/dev/null 2>&1 || pgrep -f clash >/dev/null 2>&1 ;;
        surge)        pgrep -f Surge >/dev/null 2>&1 ;;
        auto)         pgrep -f verge-mihomo >/dev/null 2>&1 || shadowrocket_up \
                      || pgrep -f sing-box >/dev/null 2>&1 \
                      || pgrep -x ClashX >/dev/null 2>&1 || pgrep -f clash >/dev/null 2>&1 \
                      || pgrep -f Surge >/dev/null 2>&1 ;;
        *)            pgrep -f verge-mihomo >/dev/null 2>&1 ;;
    esac
}

# 置顶规则配置检查(仅 Clash Verge):合并配置里是否含置顶规则
merge_ok() {
    grep -q "DOMAIN-SUFFIX,claude.ai,$PROXY_GROUP" "$CLASH_DIR/clash-verge.yaml" 2>/dev/null
}

# 置顶规则行为检测:三个生态域名全部解析到 fake-ip 段(198.18.x.x)即视为已置顶;未命中域名记入 ROUTE_MISSED
route_ok() {
    ROUTE_MISSED=""
    local d a
    for d in claude.ai api.anthropic.com claudeusercontent.com; do
        a=$(dig +short +time=3 +tries=1 "$d" 2>/dev/null | head -1)
        case "$a" in
            198.18.*) ;;
            *) ROUTE_MISSED="$ROUTE_MISSED $d" ;;
        esac
    done
    [ -z "$ROUTE_MISSED" ]
}

# 国家代码 → 地区格式映射(未收录的国家回退 en_US)
locale_for() {
    case "$1" in
        US) echo en_US ;; JP) echo ja_JP ;; TW) echo zh_TW ;; HK) echo zh_HK ;;
        SG) echo en_SG ;; GB) echo en_GB ;; DE) echo de_DE ;; FR) echo fr_FR ;;
        KR) echo ko_KR ;; AU) echo en_AU ;; CA) echo en_CA ;; *) echo en_US ;;
    esac
}

# ---------- 安全检查 ----------
check() {
    fails=""

    proxy_alive || fails="$fails;代理内核未运行"
    tun_up || fails="$fails;TUN网卡不在线"

    trace=$(curl -s --max-time 8 https://www.cloudflare.com/cdn-cgi/trace 2>/dev/null)
    ip=$(echo "$trace" | grep '^ip=' | cut -d= -f2)
    loc=$(echo "$trace" | grep '^loc=' | cut -d= -f2)
    [ "$loc" = "$COUNTRY_TARGET" ] || fails="$fails;出口IP异常(${ip:-无}/${loc:-未知},期望$COUNTRY_TARGET)"
    ipwarn=""
    lastip=""; [ -f "$BASE/lastip" ] && lastip=$(cat "$BASE/lastip")
    if [ -n "$ip" ] && [ -n "$lastip" ] && [ "$ip" != "$lastip" ]; then
        ipwarn="注意:出口IP与上次不同($lastip -> $ip)"
    fi
    [ -n "$ip" ] && echo "$ip" > "$BASE/lastip"

    tz=$(readlink /etc/localtime 2>/dev/null)
    [ "$tz" = "/var/db/timezone/zoneinfo/$TZ_TARGET" ] || fails="$fails;时区异常($tz)"

    locale=$(defaults read -g AppleLocale 2>/dev/null)
    [ "$locale" = "$LOCALE_TARGET" ] || fails="$fails;地区异常($locale)"

    v6=$(networksetup -getinfo Wi-Fi 2>/dev/null | grep -i '^IPv6:')
    echo "$v6" | grep -qi 'off' || fails="$fails;IPv6未关闭"

    dnsip=$(dig +short +time=3 +tries=1 claude.ai 2>/dev/null | head -1)
    case "$dnsip" in
        198.18.*) ;;
        *) fails="$fails;DNS未走隧道(返回${dnsip:-空})" ;;
    esac

    wr=$(webrtc_test)
    if [ "$wr" != "RESULT:NONE" ]; then
        case "$wr" in
            RESULT:*)
                wips=${wr#RESULT:}
                leak=""
                OLDIFS=$IFS; IFS=','
                for w in $wips; do [ "$w" != "$ip" ] && leak="$leak $w"; done
                IFS=$OLDIFS
                [ -n "$leak" ] && fails="$fails;WebRTC泄漏($leak)" ;;
            *) fails="$fails;WebRTC检测异常" ;;
        esac
    fi

    launchctl print system/local.claude-bodyguard >/dev/null 2>&1 || fails="$fails;保镖未运行"

    # 置顶规则:Clash Verge 看配置或行为任一通过;其他客户端只看行为检测
    if [ "$PROXY_MODE" = "clash-verge" ]; then
        merge_ok || route_ok || fails="$fails;置顶规则缺失"
    else
        route_ok || fails="$fails;置顶规则缺失(未命中:${ROUTE_MISSED# })"
    fi

    defaults read com.anthropic.claudefordesktop AppleLanguages 2>/dev/null | grep -q 'en-US' || {
        syslang=$(defaults read -g AppleLanguages 2>/dev/null | sed -n '2p' | tr -d ' ,"')
        [[ "$syslang" == en* ]] || fails="$fails;客户端语言为中文"
    }

    r1=$(curl -s --max-time 8 -o /dev/null -w '%{http_code}' https://claude.ai/ 2>/dev/null)
    r2=$(curl -s --max-time 8 -o /dev/null -w '%{http_code}' https://api.anthropic.com/v1/models 2>/dev/null)
    { [ "$r1" = "000" ] || [ "$r2" = "000" ]; } && fails="$fails;Anthropic生态不可达($r1/$r2)"

    if [ -n "$fails" ]; then
        echo "FAIL:${fails#;}"
        return 1
    fi
    echo "OK:$ip${ipwarn:+|$ipwarn}"
    return 0
}

# WebRTC/STUN 探测:返回 RESULT:ip,ip 或 RESULT:NONE 或空
webrtc_test() {
    python3 "$BASE/stun-probe.py" 2>/dev/null
}

# 逐项输出检查结果(供UI使用): 名称=ok|fail:细节
checkdetail() {
    if proxy_alive; then
        case "$PROXY_MODE" in
            shadowrocket) echo '代理内核=ok:Shadowrocket 已连接' ;;
            *) echo "代理内核=ok:$(proxy_name) 运行中" ;;
        esac
    else
        echo "代理内核=fail:$(proxy_name) 未运行"
    fi
    if tun_up; then echo 'TUN网卡=ok'; else echo 'TUN网卡=fail:不在线'; fi
    trace=$(curl -s --max-time 8 https://www.cloudflare.com/cdn-cgi/trace 2>/dev/null)
    ip=$(echo "$trace" | grep '^ip=' | cut -d= -f2)
    loc=$(echo "$trace" | grep '^loc=' | cut -d= -f2)
    if [ "$loc" = "$COUNTRY_TARGET" ]; then echo "出口IP=ok:$ip/$loc"; else echo "出口IP=fail:${ip:-无}/${loc:-未知}(期望$COUNTRY_TARGET)"; fi
    lastip=""; [ -f "$BASE/lastip" ] && lastip=$(cat "$BASE/lastip")
    if [ -z "$ip" ]; then echo 'IP一致=fail:未获取到出口IP'
    elif [ -z "$lastip" ]; then echo 'IP一致=ok:首次记录'
    elif [ "$ip" = "$lastip" ]; then echo 'IP一致=ok'
    else echo "IP一致=warn:$lastip → $ip"; fi
    [ -n "$ip" ] && echo "$ip" > "$BASE/lastip"
    tz=$(readlink /etc/localtime 2>/dev/null)
    if [ "$tz" = "/var/db/timezone/zoneinfo/$TZ_TARGET" ]; then echo '时区=ok'; else echo "时区=fail:$tz"; fi
    locale=$(defaults read -g AppleLocale 2>/dev/null)
    if [ "$locale" = "$LOCALE_TARGET" ]; then echo '地区=ok'; else echo "地区=fail:$locale"; fi
    v6=$(networksetup -getinfo Wi-Fi 2>/dev/null | grep -i '^IPv6:')
    if echo "$v6" | grep -qi 'off'; then echo 'IPv6=ok:已关闭'; else echo 'IPv6=fail:未关闭'; fi
    dnsip=$(dig +short +time=3 +tries=1 claude.ai 2>/dev/null | head -1)
    case "$dnsip" in
        198.18.*) echo 'DNS=ok:走隧道' ;;
        *) echo "DNS=fail:未走隧道(${dnsip:-空})" ;;
    esac
    wr=$(webrtc_test)
    case "$wr" in
        RESULT:NONE) echo 'WebRTC=ok:UDP不出网,零暴露' ;;
        RESULT:*)
            wips=${wr#RESULT:}
            leak=""
            OLDIFS=$IFS; IFS=','
            for w in $wips; do [ "$w" != "$ip" ] && leak="$leak $w"; done
            IFS=$OLDIFS
            if [ -n "$leak" ]; then echo "WebRTC=fail:暴露非代理IP:$leak"; else echo 'WebRTC=ok:仅暴露代理IP'; fi ;;
        *) echo 'WebRTC=fail:检测异常' ;;
    esac
    if launchctl print system/local.claude-bodyguard >/dev/null 2>&1; then
        lastkill=$(tail -1 "$HOME/Library/Logs/claude-bodyguard.log" 2>/dev/null | cut -c1-40)
        echo "保镖=ok:${lastkill:-待命}"
    else
        echo '保镖=fail:未安装'
    fi
    # 置顶规则:Clash Verge 配置/行为双保险,其他客户端只看行为
    if [ "$PROXY_MODE" = "clash-verge" ]; then
        if merge_ok; then
            echo '规则置顶=ok'
        elif route_ok; then
            echo '规则置顶=ok:行为检测通过'
        else
            echo '规则置顶=fail:未生效'
        fi
    else
        if route_ok; then
            echo '规则置顶=ok:行为检测通过'
        else
            echo "规则置顶=fail:未命中(${ROUTE_MISSED# }),请在 $(proxy_name) 中自行加置顶规则"
        fi
    fi
    # 12. Claude 客户端语言(单独设为英文,或系统首选语言为英文均可;防止 JS 层读出中文)
    clang=$(defaults read com.anthropic.claudefordesktop AppleLanguages 2>/dev/null | head -2)
    syslang=$(defaults read -g AppleLanguages 2>/dev/null | sed -n '2p' | tr -d ' ,"')
    if echo "$clang" | grep -q 'en-US'; then
        echo '客户端语言=ok:en-US'
    elif [[ "$syslang" == en* ]]; then
        echo '客户端语言=ok:跟随系统(英文)'
    else
        echo '客户端语言=fail:跟随系统(中文)'
    fi
    # 13. ASN 网络属性(住宅宽带 vs 机房,按目标地区收录主流运营商)
    org=$(curl -s --max-time 6 https://ipinfo.io/json 2>/dev/null | grep '"org"' | cut -d'"' -f4)
    case "$org" in
        '') echo 'ASN属性=warn:未获取' ;;
        *Comcast*|*AT\&T*|*Verizon*|*Spectrum*|*Charter*|*Cox*|*T-Mobile*|*Cable*|*esidential*|\
        *NTT*|*KDDI*|*SoftBank*|*IIJ*|*OCN*|*So-net*|\
        *HiNet*|*Chunghwa*|*CHT*|*kbro*|*Taiwan*|*FarEasTone*|*Data\ Communication*|\
        *Singtel*|*StarHub*|*M1*|\
        *HKT*|*PCCW*|*HKBN*|*HGC*|\
        *BT*|*Sky*|*Virgin*|*TalkTalk*|*Vodafone*|\
        *Deutsche\ Telekom*|*1\&1*|*O2*) echo "ASN属性=ok:$org" ;;
        *) echo "ASN属性=warn:疑似机房($org)" ;;
    esac
    # 14. Anthropic 生态可达性
    r1=$(curl -s --max-time 8 -o /dev/null -w '%{http_code}' https://claude.ai/ 2>/dev/null)
    r2=$(curl -s --max-time 8 -o /dev/null -w '%{http_code}' https://api.anthropic.com/v1/models 2>/dev/null)
    if [ "$r1" != "000" ] && [ "$r2" != "000" ]; then
        echo "生态可达性=ok:claude.ai:$r1 api:$r2"
    else
        echo "生态可达性=fail:claude.ai:$r1 api:$r2"
    fi
}

# 一键修复:按项自动修复,每步输出结果
fix() {
    echo "FIX:开始一键修复"

    # A. 代理内核 + TUN
    if [ "$PROXY_MODE" = "clash-verge" ]; then
        if ! pgrep -f verge-mihomo >/dev/null 2>&1; then
            echo "FIX:正在启动 Clash Verge..."
            open -a "Clash Verge"
        fi
        for i in $(seq 1 40); do
            pgrep -f verge-mihomo >/dev/null 2>&1 && tun_up && break
            sleep 1
        done
        if pgrep -f verge-mihomo >/dev/null 2>&1 && tun_up; then
            echo "FIX:代理内核+TUN ✓"
        else
            V="$CLASH_DIR/verge.yaml"
            if [ -f "$V" ] && grep -q '^enable_tun_mode: false' "$V"; then
                echo "FIX:检测到 TUN 开关被关闭,正在恢复..."
                osascript -e 'tell application "Clash Verge" to quit' >/dev/null 2>&1
                sleep 3
                sed -i '' 's/^enable_tun_mode: false/enable_tun_mode: true/' "$V"
                open -a "Clash Verge"
                for i in $(seq 1 40); do
                    tun_up && break
                    sleep 1
                done
            fi
            if pgrep -f verge-mihomo >/dev/null 2>&1 && tun_up; then
                echo "FIX:代理内核+TUN ✓(已恢复TUN开关)"
            else
                echo "FIX:代理内核/TUN ✗(请打开 Clash Verge 检查节点/线路)"
            fi
        fi
    else
        app=$(proxy_app_name)
        if ! proxy_alive && [ -n "$app" ]; then
            echo "FIX:正在启动 $app..."
            open -a "$app" >/dev/null 2>&1
        fi
        for i in $(seq 1 40); do
            proxy_alive && tun_up && break
            sleep 1
        done
        if proxy_alive && tun_up; then
            echo "FIX:代理内核+TUN ✓"
        else
            echo "FIX:代理内核/TUN ✗(请打开 $(proxy_name) 手动检查节点/线路及TUN模式)"
        fi
    fi

    # B. 时区/IPv6/DNS缓存/保镖(需管理员权限,会弹一次密码框)
    tz=$(readlink /etc/localtime 2>/dev/null)
    v6=$(networksetup -getinfo Wi-Fi 2>/dev/null | grep -i '^IPv6:')
    adminCmds=""
    [ "$tz" != "/var/db/timezone/zoneinfo/$TZ_TARGET" ] && adminCmds="${adminCmds}/usr/sbin/systemsetup -settimezone $TZ_TARGET; "
    echo "$v6" | grep -qi 'off' || adminCmds="${adminCmds}networksetup -setv6off Wi-Fi; "
    if ! launchctl print system/local.claude-bodyguard >/dev/null 2>&1; then
        adminCmds="${adminCmds}cp $HOME/.local/bin/local.claude-bodyguard.plist /Library/LaunchDaemons/ && chown root:wheel /Library/LaunchDaemons/local.claude-bodyguard.plist && launchctl bootstrap system /Library/LaunchDaemons/local.claude-bodyguard.plist; "
    fi
    if [ -n "$adminCmds" ]; then
        adminCmds="${adminCmds}dscacheutil -flushcache; killall -HUP mDNSResponder"
        echo "FIX:修复时区/IPv6/DNS/保镖(需要密码)..."
        if osascript <<ASEOF >/dev/null 2>&1
do shell script "$adminCmds" with prompt "一键修复需要管理员权限" with administrator privileges
ASEOF
        then
            echo "FIX:时区/IPv6/DNS/保镖 ✓"
        else
            echo "FIX:时区/IPv6/保镖 ✗(取消或失败)"
        fi
    else
        echo "FIX:时区/IPv6/保镖 无需修复 ✓"
    fi

    # B3. Claude 客户端单独设英文(用户级,无需密码;系统首选语言已是英文则跳过)
    syslang=$(defaults read -g AppleLanguages 2>/dev/null | sed -n '2p' | tr -d ' ,"')
    if ! defaults read com.anthropic.claudefordesktop AppleLanguages 2>/dev/null | grep -q 'en-US' && [[ "$syslang" != en* ]]; then
        defaults write com.anthropic.claudefordesktop AppleLanguages -array en-US
        echo "FIX:Claude 客户端语言 → en-US ✓(重启 Claude 后生效)"
    fi

    # B4. 地区格式(用户级,无需密码)
    locale=$(defaults read -g AppleLocale 2>/dev/null)
    if [ "$locale" != "$LOCALE_TARGET" ]; then
        defaults write -g AppleLocale "$LOCALE_TARGET"
        echo "FIX:地区格式 → $LOCALE_TARGET ✓"
    fi

    # B2. 置顶规则缺失 → Clash Verge 重写 Merge 配置并重启;其他客户端提示自行配置
    if [ "$PROXY_MODE" = "clash-verge" ]; then
        if ! merge_ok && ! route_ok; then
            echo "FIX:重写置顶规则..."
            cat > "$CLASH_DIR/profiles/Merge.yaml" <<MEOF
# Profile Enhancement Merge Template for Clash Verge

profile:
  store-selected: true

# Claude 全家族及生态域名置顶强制走代理(优先级高于任何国内规则集)
prepend-rules:
  - DOMAIN-SUFFIX,claude.ai,$PROXY_GROUP
  - DOMAIN-SUFFIX,claude.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,anthropic.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,claudeusercontent.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,challenges.cloudflare.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,datadoghq.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,segment.io,$PROXY_GROUP
  - DOMAIN-SUFFIX,stripe.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,stripe.network,$PROXY_GROUP
  - DOMAIN-SUFFIX,stripecdn.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,intercom.io,$PROXY_GROUP
  - DOMAIN-SUFFIX,intercomcdn.com,$PROXY_GROUP
  - DOMAIN-SUFFIX,intercomassets.com,$PROXY_GROUP
MEOF
            osascript -e 'tell application "Clash Verge" to quit' >/dev/null 2>&1
            sleep 3
            open -a "Clash Verge"
            for i in $(seq 1 40); do
                tun_up && break
                sleep 1
            done
            grep -q "DOMAIN-SUFFIX,claude.ai,$PROXY_GROUP" "$CLASH_DIR/clash-verge.yaml" 2>/dev/null \
                && echo "FIX:置顶规则 ✓" || echo "FIX:置顶规则 ✗"
        fi
    else
        if ! route_ok; then
            echo "FIX:置顶规则 ✗(未命中:${ROUTE_MISSED# }) — 请在 $(proxy_name) 中自行添加 DOMAIN-SUFFIX 置顶规则"
        fi
    fi

    # C. 出口IP最终验证
    sleep 2
    trace=$(curl -s --max-time 8 https://www.cloudflare.com/cdn-cgi/trace 2>/dev/null)
    ip=$(echo "$trace" | grep '^ip=' | cut -d= -f2)
    loc=$(echo "$trace" | grep '^loc=' | cut -d= -f2)
    if [ "$loc" = "$COUNTRY_TARGET" ]; then
        echo "FIX:出口IP ✓ ($ip/$loc)"
    else
        echo "FIX:出口IP ✗ (${ip:-无}/${loc:-未知},期望$COUNTRY_TARGET) — 可能是节点故障,请到 $(proxy_name) 里换条线路"
    fi
    echo "FIX:修复流程结束"
}

# 地区自适应:按当前出口国家一键重设 国家/时区/地区格式(换了国家节点后运行)
set_region() {
    trace=$(curl -s --max-time 8 https://www.cloudflare.com/cdn-cgi/trace 2>/dev/null)
    loc=$(echo "$trace" | grep '^loc=' | cut -d= -f2)
    [ -z "$loc" ] && { echo "FAIL:无法获取出口国家(cloudflare trace 无结果,请先挂代理)"; return 1; }
    info=$(curl -s --max-time 8 https://ipinfo.io/json 2>/dev/null)
    ctry=$(echo "$info" | grep '"country"' | cut -d'"' -f4)
    ntz=$(echo "$info" | grep '"timezone"' | cut -d'"' -f4)
    if [ -z "$ctry" ] || [ -z "$ntz" ]; then
        echo "FAIL:ipinfo 未返回国家或时区"; return 1
    fi
    if [ "$loc" != "$ctry" ]; then
        echo "FAIL:出口国家不一致(cloudflare=$loc ipinfo=$ctry),已中止"; return 1
    fi
    nlocale=$(locale_for "$ctry")

    # 改写脚本自身头部三个配置项
    sed -i '' \
        -e "s|^COUNTRY_TARGET=\".*\"|COUNTRY_TARGET=\"$ctry\"|" \
        -e "s|^TZ_TARGET=\".*\"|TZ_TARGET=\"$ntz\"|" \
        -e "s|^LOCALE_TARGET=\".*\"|LOCALE_TARGET=\"$nlocale\"|" \
        "$0"
    COUNTRY_TARGET="$ctry"; TZ_TARGET="$ntz"; LOCALE_TARGET="$nlocale"

    # 地区格式(用户级,无需密码)
    defaults write -g AppleLocale "$nlocale"

    # 时区(需管理员权限,会弹一次授权框)
    if ! osascript <<ASEOF >/dev/null 2>&1
do shell script "/usr/sbin/systemsetup -settimezone $ntz" with prompt "set-region 需要管理员权限以锁定时区" with administrator privileges
ASEOF
    then
        echo "FAIL:时区设置被取消或失败(国家/地区格式已更新为 $ctry/$nlocale)"
        return 1
    fi

    # 同步时区看门狗的目标值
    [ -f "$HOME/.local/bin/tz-watchdog.sh" ] && sed -i '' "s|^TARGET=.*|TARGET=\"$ntz\"|" "$HOME/.local/bin/tz-watchdog.sh"

    echo "OK:region=$loc tz=$ntz locale=$nlocale"
}

status() {
    cur="none"; [ -f "$BASE/current" ] && cur=$(cat "$BASE/current")
    hasA=0; [ -d "$BASE/accounts/A" ] && hasA=1
    hasB=0; [ -d "$BASE/accounts/B" ] && hasB=1
    echo "current=$cur hasA=$hasA hasB=$hasB"
}

quit_claude() {
    pgrep -f "/Applications/Claude.app/Contents/MacOS/Claude" >/dev/null 2>&1 || return 0
    osascript -e 'tell application "Claude" to quit' >/dev/null 2>&1
    for i in $(seq 1 15); do
        pgrep -f "/Applications/Claude.app/Contents/MacOS/Claude" >/dev/null 2>&1 || return 0
        sleep 1
    done
    return 1
}

archive_current() {
    cur="$1"
    rm -rf "$BASE/accounts/$cur.tmp"
    ditto "$DATA" "$BASE/accounts/$cur.tmp" && rm -rf "$BASE/accounts/$cur" && mv "$BASE/accounts/$cur.tmp" "$BASE/accounts/$cur"
    log "已存档账号$cur"
}

switch_to() {
    target="$1"
    cur="none"; [ -f "$BASE/current" ] && cur=$(cat "$BASE/current")

    # 首次运行:把当前登录中的状态收编为账号A
    if [ "$cur" = "none" ] && [ -d "$DATA" ] && [ -n "$(ls -A "$DATA" 2>/dev/null)" ]; then
        quit_claude || { echo "FAIL:Claude 未能退出，请手动退出后重试"; return 1; }
        archive_current A
        cur="A"; echo "$cur" > "$BASE/current"
        log "首次运行:当前登录状态已收编为账号A"
        echo "NOTE:已将当前登录收编为账号A"
    fi

    if [ "$cur" = "$target" ]; then
        open -a "Claude"
        echo "INFO:当前已是账号$target"
        return 0
    fi

    quit_claude || { echo "FAIL:Claude 未能退出，请手动退出后重试"; return 1; }

    [ "$cur" != "none" ] && [ -d "$DATA" ] && archive_current "$cur"

    rm -rf "$DATA.new"
    if [ -d "$BASE/accounts/$target" ]; then
        ditto "$BASE/accounts/$target" "$DATA.new"
        log "已载入账号$target 存档"
    else
        mkdir -p "$DATA.new"
        log "账号$target 无存档，将以全新状态初始化"
    fi
    rm -rf "$DATA" && mv "$DATA.new" "$DATA"
    echo "$target" > "$BASE/current"

    open -a "Claude"
    log "已切换到账号$target 并启动"
    echo "OK:已切换到账号$target"
    return 0
}

case "${1:-}" in
    check)  check ;;
    checkdetail)
        OUT_FILE="$BASE/last-check.txt"
        {
            echo "# $(date '+%F %T') pid=$$ 开始"
            checkdetail
            echo "# 结束"
        } > "$OUT_FILE" 2>&1
        cat "$OUT_FILE"
        ;;
    fix) fix ;;
    status) status ;;
    switch) switch_to "${2:-}" ;;
    set-region) set_region ;;
    *) echo "用法: claude-switch.sh check|checkdetail|fix|status|switch A|B|set-region"; exit 2 ;;
esac
