import assert from 'node:assert/strict';
import {readFileSync, writeFileSync, unlinkSync} from 'node:fs';
import {test} from 'node:test';
import vm from 'node:vm';
import {compile} from 'svelte/compiler';
import {render} from 'svelte/server';
import ts from 'typescript';
import * as ui from '../src/ui.ts';

const source = readFileSync(new URL('../src/App.svelte', import.meta.url), 'utf8');
// Compile the production recovery fragments, rather than reproducing their expressions.
async function recoveryView(field) {
  const fragment = source.match(new RegExp(`\\{#if ${field}\\.recovery\\}[^]*?\\{/if\\}`))?.[0];
  assert.ok(fragment, `${field} must retain recovery guidance`);
  const component = `<script>import * as ui from '../src/ui.ts'; const recoveryMessage=ui.recoveryMessage; let {${field}}=$props();</script>${fragment}`;
  const file = new URL(`./.recovery-copy-${field}-${process.pid}.mjs`, import.meta.url);
  writeFileSync(file, compile(component, {generate:'server', filename:'Recovery.svelte'}).js.code);
  try {
    const {default:View} = await import(file.href);
    return recovery => render(View, {props:{[field]:{completed:false,message:'等待处理',recovery}}}).body;
  } finally { unlinkSync(file); }
}

for (const field of ['outcome']) {
  test(`${field}: production presentation translates codes and preserves Chinese guidance`, async () => {
    const view = await recoveryView(field);
    const cases = [
      ['wait_release_materials','请等待正式更新材料就绪，然后重新检查更新。'],
      ['update_recovery_required','请核验已安装组件及兼容回退工件，再恢复更新事务。'],
      ['clock_recovery_required','请校正系统时间，重新建立时间基线后重试。'],
      ['lifecycle_busy','请等待当前任务完成后重试。'],
      ['update_in_progress','请等待当前更新任务完成后重试。'],
      ['future_internal_code','请查看活动记录中的原因，完成处理后重试。'],
      ['请先完成备份验证。','请先完成备份验证。'],
    ];
    for (const [value, expected] of cases) {
      const html = view(value);
      assert.ok(html.includes(expected), `${field}: ${value} should show ${expected}; got ${html}`);
      if (!/[\u3400-\u9fff]/.test(value)) assert.ok(!html.includes(value), 'never echo an internal recovery code');
    }
    assert.doesNotMatch(view(null), /<p[ >]/, 'null guidance has no paragraph');
    assert.doesNotMatch(view(''), /<p[ >]/, 'empty guidance has no paragraph');
  });
}

test('formatting preserves the outcome contract and the ad confirmation discriminator', () => {
  const result={completed:false,message:'等待材料',recovery:'wait_release_materials'};
  assert.equal(typeof ui.recoveryMessage,'function');
  ui.recoveryMessage(result.recovery);
  assert.deepEqual(result,{completed:false,message:'等待材料',recovery:'wait_release_materials'});
  assert.match(source, /result\.recovery==='confirm_ad_link'/);
});

test('fixture update and account actions make zero IPC calls', async () => {
  const script=source.split('<script lang="ts">')[1].split('</script>')[0]
    .replace(/^\s*import .*?;\s*$/gm,'')
    .replaceAll('import.meta.env',"({DEV:true,VITE_EDITION:'personal',VITE_IPC_SMOKE:'0'})");
  let calls=0;
  const sandbox={invoke:async()=>{calls++;throw Error('fixture IPC forbidden');},slots:[],...ui,
    $state:v=>v,$derived:v=>v,$effect:()=>{},onMount:()=>{},tick:async()=>{},
    location:{search:'?fixture=1'},URLSearchParams,document:{getElementById:()=>null},Set,console};
  vm.runInNewContext(ts.transpileModule(`${script}\nglobalThis.test={perform,get outcome(){return outcome}};`,
    {compilerOptions:{target:ts.ScriptTarget.ES2022,module:ts.ModuleKind.ESNext}}).outputText,sandbox);
  await sandbox.test.perform('software_update',{action:'check'});
  await sandbox.test.perform('account_action',{action:'select_b'});
  assert.equal(calls,0);
  assert.equal(sandbox.test.outcome.completed,false);
  assert.equal(sandbox.test.outcome.recovery,null);
});
