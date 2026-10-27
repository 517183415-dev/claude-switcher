(async () => {
  const plan = JSON.parse(document.body.dataset.plan);
  const rows = [];
  const require = (value, message) => { if (!value) throw new Error(message); };
  const transfer = async (route, body) => {
    const response = await fetch(`${plan.endpoint}/${route}`, {
      method: 'POST', headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token},
      body: JSON.stringify(body ?? {})
    });
    require(response.ok, `fixture transfer failed: ${route}`);
    return response;
  };
  const call = async (label, command, args = {}, reject = null) => {
    try {
      const value = await window.__TAURI_INTERNALS__.invoke(command, args);
      require(reject === null, `expected rejection: ${label}`);
      rows.push({label, command, ok: true, value});
      return value;
    } catch (error) {
      if (reject === null || !String(error).includes(reject)) throw error;
      rows.push({label, command, ok: false, error: String(error)});
    }
  };
  const published = async (label, channel) => {
    const value = await call(label, 'publish_ad', {channel});
    require(value.completed === true && value.message.includes('公开回读验证'), label);
  };
  let report;
  try {
    require(location.protocol === 'tauri:' && location.hostname === 'localhost', 'local protocol required');
    require((await call('runtime', 'runtime_info')).edition === 'personal', 'edition');
    const draft = await call('initial', 'ad_draft_read');
    require((await call('key', 'create_ad_signing_key')).completed === true, 'key');
    await call('duplicate-key', 'create_ad_signing_key', {}, 'signing_key_already_configured');
    const imageResponse = await fetch('image.png');
    require(imageResponse.ok, 'image fixture');
    const image = await call('image', 'import_ad_image', {bytes: Array.from(new Uint8Array(await imageResponse.arrayBuffer()))});
    const now = Math.floor(Date.now() / 1000);
    draft.slots.forEach((slot, i) => {
      slot.campaigns = [0, 1].map(j => ({
        campaign_id: `synthetic-${i}-${j}`, merchant_id: 'synthetic-merchant',
        merchant_name: '合成商家', title: `合成广告 ${i}/${j}`, description: '仅本机测试',
        image: image.asset, button_text: '查看', target_url: 'https://synthetic.example.com/',
        starts_at: now + j * 3600, ends_at: now + (j + 1) * 3600, enabled: true
      }));
    });
    const saved = await call('save', 'ad_draft_save', {manifest: draft});
    await published('preview', 'preview');
    await published('production', 'production');
    const passphrase = 'synthetic-publisher-backup-password';
    require((await call('backup', 'export_publisher_backup', {passphrase})).completed === true, 'backup');
    await transfer('checkpoint/saved');
    const backup = Array.from(new Uint8Array(await (await transfer('backup')).arrayBuffer()));
    await call('pause', 'pause_ad', {slot: null});
    await published('paused-production', 'production');
    await transfer('checkpoint/paused');
    await call('wrong-password', 'restore_publisher_backup', {bytes: backup, passphrase: 'incorrect-synthetic-password'}, 'backup');
    await transfer('checkpoint/wrong-password');
    const restored = await call('restore', 'restore_publisher_backup', {bytes: backup, passphrase});
    require(restored.completed === false && restored.message.includes('尚未发布'), 'restore must await publication');
    const restoredDraft = await call('restored-draft', 'ad_draft_read');
    require(JSON.stringify(restoredDraft.slots) === JSON.stringify(saved.slots), 'restored slots');
    require((await call('recovery-status', 'publisher_status')).status === 'recovery_required', 'recovery gate');
    await transfer('checkpoint/restored');
    await published('republished', 'production');
    require((await call('ready-status', 'publisher_status')).status === 'ready_local', 'ready status');
    await transfer('checkpoint/republished');
    report = {ok: true, rows, url: location.href, userAgent: navigator.userAgent};
  } catch (error) {
    report = {ok: false, rows, error: String(error), url: location.href, userAgent: navigator.userAgent};
  }
  document.body.textContent = `原生运营验证：${report.ok ? '通过' : '失败'}，${rows.length} 次响应`;
  await transfer('finish', report);
})();
