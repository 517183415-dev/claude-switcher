(() => {
  window.__remoteUiProbeStarted = true;
  const plan = JSON.parse(document.querySelector('meta[name="native-plan"]').content);
  const rows = window.__remoteUiRows = [];
  const failEarly = error => fetch(`${plan.endpoint}/finish`, {method: 'POST',
    headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token},
    body: JSON.stringify({ok: false, rows, error: String(error)})});
  window.addEventListener('error', event => { void failEarly(event.message); }, {once: true});
  const require = (value, message) => { if (!value) throw new Error(message); };
  const wait = async (test, label) => {
    const deadline = Date.now() + 15000;
    while (Date.now() < deadline) { const value = test(); if (value) return value; await new Promise(resolve => setTimeout(resolve, 40)); }
    throw new Error(`timeout: ${label}`);
  };
  const textButton = label => [...document.querySelectorAll('button')].find(button => button.textContent.trim() === label && !button.disabled);
  const click = async label => (await wait(() => textButton(label), label)).click();
  const panel = () => [...document.querySelectorAll('section')].find(section => section.querySelector('h2')?.textContent.trim() === '发布来源');
  const field = label => [...panel().querySelectorAll('label')].find(item => item.firstChild?.textContent.trim() === label)?.querySelector('input,select');
  const set = (element, value, event) => { require(element, `missing field: ${value}`); element.value = value; element.dispatchEvent(new Event(event, {bubbles: true})); };
  const transfer = async (route, body = {}) => {
    const response = await fetch(`${plan.endpoint}/${route}`, {method: 'POST', headers: {'Content-Type': 'application/json', 'X-Fixture-Token': plan.token}, body: JSON.stringify(body)});
    require(response.ok, `fixture transfer: ${route}`);
  };
  const saved = expected => wait(() => panel()?.textContent.includes(`该渠道已保存的来源：${expected}`), expected);
  const source = async label => {
    const input = field('发布来源');
    const option = [...input.options].find(item => item.textContent.trim() === label);
    require(option, 'missing source option');
    set(input, option.value, 'change');
    if (label === 'Cloudflare R2') await wait(() => field('Cloudflare 账号 ID'), 'remote fields');
  };
  const configure = async (bucket, host, key, secret) => {
    await source('Cloudflare R2');
    for (const [label, value] of [['Cloudflare 账号 ID', 'abcdef0123456789abcdef0123456789'], ['存储桶名称', bucket], ['公开域名', `https://${host}.example.com`], ['访问密钥 ID', key], ['访问密钥', secret]]) set(field(label), value, 'input');
    await click('保存该渠道配置');
    await saved(`https://${host}.example.com`);
    await wait(() => panel().textContent.includes('配置已保存。'), 'saved message');
    require(field('访问密钥 ID').value === '' && field('访问密钥').value === '', 'credential fields not cleared');
  };
  window.addEventListener('DOMContentLoaded', async () => {
    let result;
    try {
      require(location.protocol === 'tauri:' && location.hostname === 'localhost', 'local protocol required');
      await click('广告运营');
      await click('创建本机广告签名密钥');
      await wait(() => rows.some(row => row.command === 'create_ad_signing_key' && row.ok), 'signing key');
      await click('读取已保存配置');
      await saved('本机目录');
      await transfer('checkpoint/baseline');
      await configure('ads-preview', 'preview', 'synthetic-ui-preview-access', 'synthetic-ui-preview-secret');
      set(field('渠道'), 'production', 'change');
      await saved('本机目录');
      await configure('ads-production', 'ads', 'synthetic-ui-production-access', 'synthetic-ui-production-secret');
      set(field('渠道'), 'preview', 'change');
      await saved('https://preview.example.com');
      await source('本机目录');
      await click('保存该渠道配置');
      await saved('本机目录');
      await wait(() => panel().textContent.includes('配置已保存。'), 'local saved');
      await click('选回此目标');
      await saved('https://preview.example.com');
      await wait(() => panel().textContent.includes('已选回保留的发布目标。'), 'retained target');
      await transfer('checkpoint/done');
      result = {ok: true, rows, url: location.href, userAgent: navigator.userAgent};
    } catch (error) {
      result = {ok: false, rows, error: String(error), url: location.href, userAgent: navigator.userAgent,
        alerts: [...document.querySelectorAll('[role="alert"]')].map(node => node.textContent.trim())};
    }
    await transfer('finish', result);
  });
})();
