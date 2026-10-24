import type { WorkspaceSnapshot } from './generated/WorkspaceSnapshot';
import type { CheckId } from './generated/CheckId';
import { checkNames } from './ui';
import type { RepairPreview } from './generated/RepairPreview';
import type { RegionPreset } from './generated/RegionPreset';

export function previewSnapshot(): WorkspaceSnapshot {
  const now = Math.floor(Date.now() / 1000);
  return {
    runtime: { schema_version: 1, version: '0.2.0', edition: import.meta.env.VITE_EDITION === 'personal' ? 'personal' : 'public', slots: ['residential_broadband', 'account_service', 'kyc_service'], automatic_updates: false, ads_enabled: false },
    accounts: {current: 'A', profiles: [{id:'A',state:'current',login_evidence:'合成资料，未验证真实登录'},{id:'B',state:'uninitialized',login_evidence:'尚未建立'}], claude_running:false,action_blocker:'合成预览不能执行系统或账号操作'},
    preferences: {ads:{slots_enabled:[false,false,false],request_purpose_acknowledged:false},updates_paused_until:null},
    check_report: {
      schema_version:1,observed_at:now,completed_at:now,duration_ms:850,network_generation:'fixture-only',
      checks:(Object.keys(checkNames) as CheckId[]).map(id=>({id,status:id==='asn_info'?'warning':'pass',reason:id==='asn_info'?'网络运营商信息暂未获取':'合成预览状态，不代表本机验证',observed_at:now,duration_ms:50,valid_until:now+30,network_generation:'fixture-only',evidence:['合成预览，不包含真实网络资料']})),
      summary:{passed:13,advisory:1,blocking:0},launch:{allowed:false,reasons:['合成预览不能启动 Claude']},
    },
    ads_manifest:null,update:{paused:false,state:'idle',message:'本人点击后检查，每次下载和安装单独触发。',next_check_at:null,candidate_version:null,can_download:false,can_install:false},pending_materials:[],
  };
}

export function previewRegionRepair(region: RegionPreset): RepairPreview {
  return {
    can_execute: false,
    message: '合成修复预览：展示待处理事务；真实恢复先查询系统持久结果，再完成提交或恢复原值。此预览不读取系统，也不建立可执行计划。',
    original: {country: '合成原国家', timezone: '合成原时区', locale: '合成原地区格式'},
    target: {country: `${region.toUpperCase()}（合成目标）`, timezone: '合成目标时区', locale: '合成目标地区格式'},
    network_service: '合成网络服务', original_ipv6: '合成原值', target_ipv6: '合成目标值',
    user_changes: [
      {id:'locale',state:'ready',original:'合成旧地区格式',target:'合成新地区格式',message:'合成项目：等待修改。'},
      {id:'client_language',state:'unchanged',original:'合成语言',target:'合成语言',message:'合成项目：无需修改。'},
      {id:'proxy_pinned_routes',state:'unsupported',original:'未确认',target:'Claude 域名置顶规则',message:'合成项目：需要人工确认代理规则。'},
    ],
    recovery_required: true,
  };
}
