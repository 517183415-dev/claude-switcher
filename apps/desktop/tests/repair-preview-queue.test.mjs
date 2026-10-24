import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import {test} from 'node:test';
const baseline=process.env.REPAIR_REVISION2_BASELINE==='1';
const base='../../../docs/acceptance/evidence/e06/core-repair-preview-revision2-baseline/';
const commands=readFileSync(new URL(baseline?base+'apps__desktop__src-tauri__src__commands.rs':'../src-tauri/src/commands.rs',import.meta.url),'utf8');
const bridge=readFileSync(new URL(baseline?base+'apps__desktop__src-tauri__src__bridge.rs':'../src-tauri/src/bridge.rs',import.meta.url),'utf8');
for(const name of ['preview_repair','preview_region_change']){
 test(`${name}: production registers request before spawning worker`,()=>{
  const body=commands.split(`pub async fn ${name}(`)[1].split('\n}')[0];
  const begin=body.indexOf('state.begin_repair_preview(request_id.clone())?');
  const spawn=body.indexOf('tokio::task::spawn_blocking');
  assert.ok(begin>=0&&begin<spawn,'request must be registered synchronously before worker enqueue');
 });
}
test('production worker never re-registers canceled or superseded requests',()=>{
 const worker=bridge.split('    pub fn preview_region(')[1].split('    pub fn preview_repair(')[0];
 assert.doesNotMatch(worker,/\.begin\(/);
 assert.match(worker,/\.complete\(&request_id,/);
});
