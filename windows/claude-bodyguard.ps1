# Claude 保镖 (Windows) —— 代理离线时自动关闭 Claude
# 由安装器注册为计划任务,登录时启动,隐藏窗口常驻
$ErrorActionPreference = 'SilentlyContinue'
$LOG = Join-Path $env:USERPROFILE ".claude-switcher\bodyguard.log"

while ($true) {
    $coreUp = [bool](Get-Process -Name 'verge-mihomo' -ErrorAction SilentlyContinue)
    $tunUp  = [bool](Get-NetIPAddress -AddressFamily IPv4 | Where-Object { $_.IPAddress -like '198.18.*' })
    if (-not ($coreUp -and $tunUp)) {
        $p = Get-Process -Name 'Claude' -ErrorAction SilentlyContinue
        if ($p) {
            $p | Stop-Process -Force
            "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') 代理离线(内核或TUN丢失),已自动关闭 Claude" | Out-File -Append -Encoding utf8 $LOG
        }
    }
    Start-Sleep -Seconds 10
}
