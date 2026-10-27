(async () => {
  const plan = JSON.parse(document.body.dataset.plan);
  const rows = [];
  const require = (value, message) => { if (!value) throw new Error(message); };
  const transfer = async (route, body = {}) => {
    const response = await fetch(`${plan.endpoint}/${route}`, {
      method: 'POST', headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token},
      body: JSON.stringify(body)
    });
    require(response.ok, `fixture transfer failed: ${route}`);
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
  let report;
  try {
    require(location.protocol === 'tauri:' && location.hostname === 'localhost', 'local protocol required');
    require((await call('runtime', 'runtime_info')).edition === 'personal', 'edition');
    require((await call('key', 'create_ad_signing_key')).completed === true, 'key');
    require((await call('preview', 'publish_ad', {channel: 'preview'})).completed === true, 'preview');
    await transfer('checkpoint/baseline');
    const passphrase = 'synthetic-recovery-passphrase';
    const canceled = await call('prepare-cancel', 'publisher_recovery_prepare', {passphrase});
    await transfer('checkpoint/prepared');
    await call('duplicate-prepare', 'publisher_recovery_prepare', {passphrase}, '已有恢复会话');
    await call('cancel', 'publisher_recovery_cancel');
    await call('confirm-canceled', 'publisher_recovery_confirm', {digest: canceled.digest}, '恢复会话不存在');
    await transfer('checkpoint/canceled');
    const rejected = await call('prepare-rejected', 'publisher_recovery_prepare', {passphrase});
    await call('confirm-invalid', 'publisher_recovery_confirm', {digest: '0'.repeat(64)}, 'recovery_stale_confirmation');
    await call('confirm-consumed', 'publisher_recovery_confirm', {digest: rejected.digest}, '恢复会话不存在');
    await transfer('checkpoint/rejected');
    const confirmed = await call('prepare-confirmed', 'publisher_recovery_prepare', {passphrase});
    const outcome = await call('confirm', 'publisher_recovery_confirm', {digest: confirmed.digest});
    require(outcome.completed === true && outcome.message.includes('尚未发布'), 'explicit confirmation');
    await transfer('checkpoint/confirmed');
    const retained = await call('retained', 'publisher_retained_list');
    require(retained.some(item => item.generation === confirmed.generation && item.state_verified === true), 'retained generation');
    require((await call('republish', 'publish_ad', {channel: 'preview'})).completed === true, 'republish');
    await transfer('checkpoint/republished');
    report = {ok: true, rows, url: location.href, userAgent: navigator.userAgent};
  } catch (error) {
    report = {ok: false, rows, error: String(error), url: location.href, userAgent: navigator.userAgent};
  }
  document.body.textContent = `原生恢复验证：${report.ok ? '通过' : '失败'}，${rows.length} 次响应`;
  await transfer('finish', report);
})();
