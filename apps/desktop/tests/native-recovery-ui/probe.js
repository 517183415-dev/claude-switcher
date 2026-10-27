(() => {
  const plan = JSON.parse(document.querySelector('meta[name="native-plan"]').content);
  const rows = window.__remoteUiRows = [];
  const require = (value, message) => { if (!value) throw new Error(message); };
  const wait = async (test, label) => {
    const deadline = Date.now() + 20000;
    while (Date.now() < deadline) {
      const value = test();
      if (value) return value;
      await new Promise(resolve => setTimeout(resolve, 40));
    }
    throw new Error(`timeout: ${label}`);
  };
  const button = label => [...document.querySelectorAll('button')]
    .find(item => item.textContent.trim() === label && !item.matches(':disabled'));
  const click = async label => (await wait(() => button(label), label)).click();
  const panel = () => [...document.querySelectorAll('section')]
    .find(item => item.querySelector('h2')?.textContent.trim() === '旧发布资料恢复');
  const count = command => rows.filter(row => row.command === command && row.ok).length;
  const transfer = async (route, body = {}) => {
    const response = await fetch(`${plan.endpoint}/${route}`, {method: 'POST',
      headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token},
      body: JSON.stringify(body)});
    require(response.ok, `fixture transfer: ${route}`);
  };
  const prepare = async () => {
    const input = await wait(() => panel()?.querySelector('input[type="password"]'), 'recovery password');
    input.value = 'synthetic-recovery-ui-passphrase';
    input.dispatchEvent(new Event('input', {bubbles: true}));
    await click('保存完整备份并准备恢复');
    const notice = await wait(() => [...panel().querySelectorAll('.notice')]
      .find(item => item.textContent.includes('完整备份已写入并读回校验')), 'recovery preview');
    require(document.querySelector('fieldset.recovery-guard')?.disabled, 'publication guard must be active');
    require(!button('发布到预览渠道'), 'publication must be disabled during confirmation');
    return notice.querySelector('p').textContent.trim();
  };
  const publish = async expected => {
    await click('发布到预览渠道');
    await click('确认发布');
    await wait(() => count('publish_ad') === expected, 'publication result');
    await wait(() => button('发布到预览渠道'), 'publication UI idle');
  };
  window.addEventListener('DOMContentLoaded', async () => {
    let report;
    try {
      require(location.protocol === 'tauri:' && location.hostname === 'localhost', 'local protocol required');
      await click('广告运营');
      await click('创建本机广告签名密钥');
      await wait(() => count('create_ad_signing_key') === 1, 'synthetic signing key');
      await publish(1);
      await transfer('checkpoint/baseline');
      const canceledBackup = await prepare();
      await transfer('checkpoint/prepared');
      await click('取消恢复');
      await wait(() => panel()?.textContent.includes('已取消。'), 'cancel result');
      require(!document.querySelector('fieldset.recovery-guard').disabled, 'guard released after cancel');
      require(panel().querySelector('input[type="password"]').value === '', 'password cleared after cancel');
      await transfer('checkpoint/canceled');
      const confirmedBackup = await prepare();
      require(confirmedBackup !== canceledBackup, 'separate backups');
      await click('确认保留旧资料并建立新代次');
      await wait(() => window.__recoveryReloadWaiting, 'controlled draft reload');
      require(document.querySelector('fieldset.recovery-guard').disabled, 'publication guard released before draft reload');
      require(!button('保存完整备份并准备恢复'), 'recovery controls enabled before draft reload');
      window.__rejectRecoveryReload(new Error('synthetic reload failure'));
      await wait(() => button('重新读取恢复结果'), 'reload retry');
      require(document.querySelector('fieldset.recovery-guard').disabled, 'publication guard released after failed reload');
      require(!button('发布到预览渠道'), 'publication enabled after failed reload');
      await click('重新读取恢复结果');
      await wait(() => panel()?.textContent.includes('尚未发布'), 'confirm result');
      await wait(() => !document.querySelector('fieldset.recovery-guard').disabled, 'guard released after confirm');
      require(panel().querySelector('input[type="password"]').value === '', 'password cleared after confirm');
      require(count('publish_ad') === 1, 'recovery must not publish');
      await transfer('checkpoint/confirmed');
      await publish(2);
      await transfer('checkpoint/republished');
      report = {ok: true, rows, backups: [canceledBackup, confirmedBackup], guard_during_reload: true,
        failed_reload_retry: true, url: location.href, userAgent: navigator.userAgent};
    } catch (error) {
      window.__releaseRecoveryReload?.();
      report = {ok: false, rows, error: String(error), url: location.href, userAgent: navigator.userAgent,
        alerts: [...document.querySelectorAll('[role="alert"]')].map(node => node.textContent.trim())};
    }
    await transfer('finish', report);
  });
})();
