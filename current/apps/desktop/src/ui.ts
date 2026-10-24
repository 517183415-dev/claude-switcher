import type { AdSlot } from './generated/AdSlot';
import type { CheckId } from './generated/CheckId';
import type { CheckStatus } from './generated/CheckStatus';

export const slots: AdSlot[] = ['residential_broadband', 'account_service', 'kyc_service'];
export const slotNames: Record<AdSlot, string> = {
  residential_broadband: '美国家庭宽带', account_service: '注册好的账号', kyc_service: 'KYC 服务',
};
export const checkNames: Record<CheckId, string> = {
  proxy_process: '代理客户端', tun_route: '隧道路由', egress_region: '出口地区',
  ip_continuity: '出口地址变化', timezone: '时区', locale: '地区格式', ipv6_policy: 'IPv6 策略',
  dns_route: 'DNS 路径', stun_path: 'STUN 路径', guardian: '后台守护', service_routes: '服务路由',
  client_language: '客户端语言', asn_info: '网络运营商信息', service_transport: '服务传输与访问结果',
};
export const statusNames: Record<CheckStatus, string> = {
  pass: '通过', warning: '提示', fail: '未通过', unknown: '待确认', timeout: '超时',
};
export function dateTime(value: number | null | undefined, zone?: string): string {
  if (!value) return '尚无记录';
  return new Intl.DateTimeFormat('zh-CN', {
    month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit', second: '2-digit',
    hour12: false, ...(zone ? { timeZone: zone } : {}),
  }).format(new Date(value * 1000));
}
export function hostLabel(url: string): string {
  try { return new URL(url).hostname; } catch { return '目标地址待校验'; }
}
export function errorMessage(error: unknown): string {
  const message=String(error);
  if (/__TAURI_INTERNALS__|reading 'invoke'|reading 'transformCallback'/.test(message)) return '未连接桌面核心。请在 Claude 切换器 App 中打开此页面，然后重新连接。';
  return /[\u3400-\u9fff]/.test(message) ? message : '操作未完成。请检查桌面核心连接，或在活动记录中查看具体原因。';
}

/** Translate recovery codes only at the display boundary; keep IPC results intact. */
export function recoveryMessage(recovery: string | null | undefined): string {
  if (!recovery) return '';
  switch (recovery) {
    case 'wait_release_materials': return '请等待正式更新材料就绪，然后重新检查更新。';
    case 'update_recovery_required': return '请核验已安装组件及兼容回退工件，再恢复更新事务。';
    case 'clock_recovery_required': return '请校正系统时间，重新建立时间基线后重试。';
    case 'lifecycle_busy': return '请等待当前任务完成后重试。';
    case 'update_in_progress': return '请等待当前更新任务完成后重试。';
    default: return /[\u3400-\u9fff]/.test(recovery) ? recovery : '请查看活动记录中的原因，完成处理后重试。';
  }
}

/** Keep keyboard focus inside a modal, then restore the original control. */
export function dialogFocus(node: HTMLElement, close: () => void) {
  const previous = document.activeElement as HTMLElement | null;
  const controls = () => Array.from(node.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled),a[href]'));
  queueMicrotask(() => (controls()[0] ?? node).focus());
  const keydown = (event: KeyboardEvent) => {
    if (event.key === 'Escape') { event.preventDefault(); close(); }
    if (event.key !== 'Tab') return;
    const items=controls(), first=items[0]??node, last=items.at(-1)??node;
    if (event.shiftKey && (document.activeElement===first || document.activeElement===node)) { event.preventDefault();last.focus(); }
    else if (!event.shiftKey && document.activeElement===last) { event.preventDefault();first.focus(); }
  };
  node.addEventListener('keydown', keydown);
  return { destroy() { node.removeEventListener('keydown', keydown);previous?.focus(); } };
}
