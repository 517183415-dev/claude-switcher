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
  const config = (channel, bucket, host) => ({account_id: 'abcdef0123456789abcdef0123456789',
    bucket, public_origin: `https://${host}.example.com`, channel});
  const save = (channel, value, accessKey, secretKey) => ({channel, config: value, accessKey, secretKey});
  const preview = config('preview', 'ads-preview', 'preview');
  const preview2 = config('preview', 'ads-preview-two', 'preview-two');
  const production = config('production', 'ads-production', 'ads');
  const firstKey = 'synthetic-preview-access';
  const firstSecret = 'synthetic-preview-secret';
  let report;
  try {
    require(location.protocol === 'tauri:' && location.hostname === 'localhost', 'local protocol required');
    require((await call('runtime', 'runtime_info')).edition === 'personal', 'edition');
    require((await call('key', 'create_ad_signing_key')).completed === true, 'key');
    await call('initial', 'publisher_remote_read');
    await transfer('checkpoint/baseline');
    const first = await call('preview', 'publisher_remote_save', save('preview', preview, firstKey, firstSecret));
    const firstTarget = first.targets.find(item => item.config.bucket === preview.bucket).target;
    await transfer('checkpoint/preview');
    await call('wrong-channel', 'publisher_remote_save', save('production', preview, 'synthetic-other-access', 'synthetic-other-secret'), 'r2_configuration');
    await call('shared-access', 'publisher_remote_save', save('production', production, firstKey, 'synthetic-production-secret'), 'r2_channels_require_separate_buckets_and_keys');
    await call('after-rejection', 'publisher_remote_read');
    await transfer('checkpoint/rejected');
    await call('production', 'publisher_remote_save', save('production', production, 'synthetic-production-access', 'synthetic-production-secret'));
    await call('second-preview', 'publisher_remote_save', save('preview', preview2, 'synthetic-preview-two-access', 'synthetic-preview-two-secret'));
    await call('wrong-retained-channel', 'publisher_remote_select', {channel: 'production', target: firstTarget}, 'r2_retained_target_missing');
    await call('restore-preview', 'publisher_remote_select', {channel: 'preview', target: firstTarget});
    await transfer('checkpoint/selected');
    await call('select-local', 'publisher_remote_save', save('preview', null, '', ''));
    await call('final-read', 'publisher_remote_read');
    await transfer('checkpoint/local');
    report = {ok: true, rows, url: location.href, userAgent: navigator.userAgent};
  } catch (error) {
    report = {ok: false, rows, error: String(error), url: location.href, userAgent: navigator.userAgent};
  }
  document.body.textContent = `原生远程配置验证：${report.ok ? '通过' : '失败'}，${rows.length} 次响应`;
  await transfer('finish', report);
})();
