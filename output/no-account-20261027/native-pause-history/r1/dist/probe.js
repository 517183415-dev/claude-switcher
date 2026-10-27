(async () => {
  const plan = JSON.parse(document.body.dataset.plan);
  const rows = [];
  const require = (value, message) => { if (!value) throw new Error(message); };
  const checkpoint = async phase => {
    const response = await fetch(`${plan.endpoint}/checkpoint/${phase}`, {
      method: 'POST', headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token},
      body: '{}'
    });
    require(response.ok, `checkpoint failed: ${phase}`);
  };
  const call = async (label, command, args = {}, expectedError = null) => {
    try {
      const value = await window.__TAURI_INTERNALS__.invoke(command, args);
      require(expectedError === null, `expected rejection: ${label}`);
      rows.push({label, command, ok: true, value});
      return value;
    } catch (error) {
      if (expectedError === null || !String(error).includes(expectedError)) throw error;
      rows.push({label, command, ok: false, error: String(error)});
    }
  };
  const publish = async label => {
    const result = await call(label, 'publish_ad', {channel: 'production'});
    require(result.completed && result.message.includes('公开回读验证'), label);
  };
  let report;
  try {
    require(location.protocol === 'tauri:' && location.hostname === 'localhost', 'local protocol required');
    require((await call('runtime', 'runtime_info')).edition === 'personal', 'edition');
    const draft = await call('initial', 'ad_draft_read');
    require((await call('key', 'create_ad_signing_key')).completed, 'key');
    const response = await fetch('image.png');
    require(response.ok, 'image fixture');
    const image = await call('image', 'import_ad_image', {bytes: Array.from(new Uint8Array(await response.arrayBuffer()))});
    const now = Math.floor(Date.now() / 1000);
    draft.slots.forEach((slot, i) => {
      slot.enabled = true;
      slot.campaigns = [0, 1].map(j => ({
        campaign_id: `synthetic-${i}-${j}`, merchant_id: 'synthetic-merchant',
        merchant_name: '合成商家', title: `合成广告 ${i}/${j}`, description: '仅本机测试',
        image: image.asset, button_text: '查看', target_url: 'https://synthetic.example.com/',
        starts_at: now + j * 3600, ends_at: now + (j + 1) * 3600, enabled: true
      }));
    });
    const saved = await call('save', 'ad_draft_save', {manifest: draft});
    await publish('publish-baseline');
    await checkpoint('baseline');
    await call('single-pause', 'pause_ad', {slot: 'account_service'});
    await checkpoint('single-draft');
    await publish('publish-single');
    await checkpoint('single-published');
    await call('all-pause', 'pause_ad', {slot: null});
    await checkpoint('all-draft');
    await publish('publish-all');
    await checkpoint('all-published');
    await call('unknown-history', 'restore_ad_history', {releaseId: '0'.repeat(32)}, 'history_missing');
    await checkpoint('rejected');
    await call('restore-history', 'restore_ad_history', {releaseId: saved.release_id});
    await checkpoint('history-draft');
    await publish('publish-history');
    await checkpoint('history-published');
    report = {ok: true, rows, url: location.href, userAgent: navigator.userAgent};
  } catch (error) {
    report = {ok: false, rows, error: String(error), url: location.href, userAgent: navigator.userAgent};
  }
  document.body.textContent = `原生暂停与历史恢复验证：${report.ok ? '通过' : '失败'}，${rows.length} 次响应`;
  await fetch(`${plan.endpoint}/finish`, {
    method: 'POST', headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token},
    body: JSON.stringify(report)
  });
})();
