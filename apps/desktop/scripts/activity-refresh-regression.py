#!/usr/bin/env python3
"""Bounded Chromium + mocked IPC regression; no native or account operations."""
import json
import os
from pathlib import Path
import socket
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = ROOT / 'output/playwright'
WRAPPER = Path('/Users/gua/CodexHome/skills/playwright/scripts/playwright_cli.sh')
PORT = 5189
SESSION = 'cs-activity-refresh'
BASE = f'http://127.0.0.1:{PORT}'
EXPORT = OUT / 'activity-fixture-export.json'
LOG = OUT / 'activity-native-export-regression.log'
CODE = r'''async (page) => {
 const snapshot=await page.evaluate(async()=> (await import('/src/fixture.ts')).previewSnapshot());
 await page.addInitScript((snapshot)=>{
   const entries=[];
   window.__activityRegressionCalls=[];
   window.__TAURI_INTERNALS__={invoke:async(command,args)=>{
     window.__activityRegressionCalls.push(command);
     if(command==='runtime_info')return snapshot.runtime;
     if(command==='workspace_snapshot')return snapshot;
     if(command==='activity_log')return structuredClone(entries);
     if(command==='check_environment'){
       entries.push({observed_at:1792666800,level:'info',message:'隔离回归：只读检查产生的新脱敏事件'});
       return snapshot.check_report;
     }
     if(command==='export_activity'){
       if(Object.keys(args??{}).length)throw Error('export received caller data');
       if(window.__exportFailure)throw '隔离回归：导出目录不可写';
       entries.push({observed_at:1792666801,level:'info',message:'隔离回归：导出时读取的最新脱敏事件'});
       window.__nativeExportEntries=structuredClone(entries);
       return ['/isolated/personal/exports/activity-redacted.json',structuredClone(entries)];
     }
     throw new Error('未模拟命令 '+command);
   }};
 },snapshot);
 await page.goto('BASE_URL/');
 await page.getByText('检查完成：13 项通过 · 1 项提示 · 0 项阻止启动。',{exact:true}).waitFor();
 await page.getByRole('button',{name:'活动记录',exact:true}).click();
 await page.locator('.activity-list').getByText('隔离回归：只读检查产生的新脱敏事件',{exact:true}).waitFor();
 if(await page.locator('.activity-list li').count()!==1)throw Error('opening check did not add exactly one event');
 if(await page.getByRole('button',{name:'导出脱敏记录',exact:true}).isDisabled())throw Error('opening check export disabled');
 await page.getByRole('button',{name:'环境检查',exact:true}).click();
 await page.locator('.page-toolbar').getByRole('button',{name:'重新检查',exact:true}).click();
 await page.getByText('检查完成：13 项通过 · 1 项提示 · 0 项阻止启动。',{exact:true}).waitFor();
 await page.getByRole('button',{name:'活动记录',exact:true}).click();
 await page.locator('.activity-list').getByText('隔离回归：只读检查产生的新脱敏事件',{exact:true}).first().waitFor({timeout:3000});
 if(await page.locator('.activity-list li').count()!==2)throw Error('manual check did not add exactly one event');
 const calls=await page.evaluate(()=>window.__activityRegressionCalls);
 if(calls.filter(c=>c==='activity_log').length!==2)throw Error('activity not reread on entry');
 let nativeDownloads=0;page.on('download',()=>nativeDownloads++);
 await page.evaluate(()=>{window.__blobCalls=0;URL.createObjectURL=()=>{window.__blobCalls++;throw Error('native Blob export forbidden');};});
 await page.getByRole('button',{name:'导出脱敏记录',exact:true}).click();
 await page.getByText('脱敏记录已保存到本机：/isolated/personal/exports/activity-redacted.json',{exact:true}).waitFor();
 await page.locator('.activity-list').getByText('隔离回归：导出时读取的最新脱敏事件',{exact:true}).waitFor();
 const shown=await page.locator('.activity-list li p').allTextContents();
 const exported=await page.evaluate(()=>window.__nativeExportEntries.map(x=>x.message));
 if(JSON.stringify(shown)!==JSON.stringify(exported))throw Error('display differs from exported snapshot '+JSON.stringify(shown));
 if(nativeDownloads||await page.evaluate(()=>window.__blobCalls))throw Error('native path used WebKit download');
 await page.evaluate(()=>window.__exportFailure=true);
 await page.getByRole('button',{name:'导出脱敏记录',exact:true}).click();
 await page.getByText('隔离回归：导出目录不可写',{exact:true}).waitFor();
 await page.getByRole('button',{name:'环境检查',exact:true}).click();
 await page.getByRole('button',{name:'活动记录',exact:true}).click();
 await page.locator('.activity-list').getByText('隔离回归：导出时读取的最新脱敏事件',{exact:true}).waitFor();
 console.log('PASS: native mocked export uses zero Blob/download, returns latest snapshot, reports path and errors; navigation responsive');
 await page.goto('BASE_URL/?fixture=1');
 await page.getByRole('button',{name:'活动记录',exact:true}).click();
 await page.locator('.activity-list').getByText('合成界面预览，未执行本机检查或账号操作。',{exact:true}).waitFor();
 const downloadPromise=page.waitForEvent('download');
 await page.getByRole('button',{name:'导出脱敏记录',exact:true}).click();
 await (await downloadPromise).saveAs('EXPORT_PATH');
 if(await page.evaluate(()=>window.__activityRegressionCalls.length)!==0)throw Error('fixture invoked native IPC');
}'''.replace('BASE_URL', BASE).replace('EXPORT_PATH', str(EXPORT)).replace('SCREENSHOT_PATH', str(OUT/'activity-refresh-current.png'))

def cli(*args):
    result=subprocess.run([str(WRAPPER),f'-s={SESSION}',*args],cwd=ROOT,capture_output=True,text=True,timeout=70)
    output=result.stdout+result.stderr
    with LOG.open('a') as log: log.write(output+'\n')
    if result.returncode or '### Error' in output:
        raise RuntimeError(output)

if __name__=='__main__':
    OUT.mkdir(parents=True, exist_ok=True)
    LOG.write_text('Chromium mocked IPC; no native side effects.\n')
    EXPORT.unlink(missing_ok=True)
    with socket.socket() as probe:
        if probe.connect_ex(('127.0.0.1',PORT))==0:raise SystemExit(f'Port {PORT} occupied; will not reuse or stop another server')
    env={**os.environ,'VITE_EDITION':'personal','VITE_IPC_SMOKE':'0'}
    with (OUT/'activity-refresh-vite.log').open('w') as vite_log:
        server=subprocess.Popen(['npm','--prefix','apps/desktop','run','dev','--','--port',str(PORT),'--strictPort'],cwd=ROOT,env=env,stdout=vite_log,stderr=subprocess.STDOUT,start_new_session=True)
        try:
            for _ in range(100):
                with socket.socket() as probe:
                    if probe.connect_ex(('127.0.0.1',PORT))==0:break
                if server.poll() is not None:raise RuntimeError('Vite exited before ready')
                time.sleep(.1)
            else:raise RuntimeError('Vite readiness timeout')
            cli('open',BASE+'/?fixture=1')
            cli('snapshot')
            cli('run-code',CODE)
            exported=json.loads(EXPORT.read_text())
            assert len(exported)==1 and exported[0]['message']=='合成界面预览，未执行本机检查或账号操作。',exported
            with LOG.open('a') as log:log.write('PASS: opening check adds one event → manual check adds one event → reenter rereads both; native mock export returns latest matching snapshot with zero Blob/download; failure remains responsive; fixture browser download invokes zero IPC.\n')
            print('PASS: activity refresh, native export IPC contract without Blob, error feedback, fixture-only browser download')
        finally:
            try:cli('close')
            finally:
                import signal
                os.killpg(server.pid,signal.SIGTERM)
                server.wait(timeout=10)
