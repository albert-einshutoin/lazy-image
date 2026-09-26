const inputs = ['coffee', 'chelsea'];
const formats = ['jpeg', 'webp'];
const report = { userAgent: navigator.userAgent, browserVersion: null, setup: [], cases: [], error: null };

try {
  if (navigator.userAgentData?.getHighEntropyValues) {
    report.browserVersion = (await navigator.userAgentData.getHighEntropyValues(['fullVersionList']))
      .fullVersionList.find((item) => item.brand === 'Google Chrome')?.version ?? null;
  }
  for (const inputId of inputs) {
    const fetched = await fetch(`/input/${inputId}`, { cache: 'no-store' });
    if (!fetched.ok) throw new Error(`input ${inputId}: HTTP ${fetched.status}`);
    const input = await fetched.arrayBuffer();
    for (const format of formats) {
      const worker = new Worker('/worker.js', { type: 'module' });
      const pending = new Map();
      let id = 0;
      worker.addEventListener('message', (event) => {
        if (event.data?.type === 'setup') {
          report.setup.push({ inputId, format, ...event.data });
          return;
        }
        const resolve = pending.get(event.data?.id);
        if (resolve) { pending.delete(event.data.id); resolve(event.data); }
      });
      worker.addEventListener('error', (event) => {
        for (const resolve of pending.values()) resolve({ ok: false, error: { message: event.message } });
        pending.clear();
      });
      const common = { format, maxWidth: 320, maxHeight: 320, fit: 'inside',
        profile: 'upload-safe', minQuality: 45, maxQuality: 85,
        qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' };
      async function run(condition, targetBytes) {
        const options = targetBytes === null ? common : { ...common, targetBytes };
        const callId = ++id;
        const startedAt = new Date().toISOString();
        let timer;
        const response = await Promise.race([
          new Promise((resolve) => { pending.set(callId, resolve);
            worker.postMessage({ id: callId, input, options }); }),
          new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`${inputId}/${format}/${condition} timeout`)), 180000); }),
        ]).finally(() => clearTimeout(timer));
        const completedAt = new Date().toISOString();
        const result = response.ok ? { ...response.result, data: undefined } : null;
        const output = response.ok ? Array.from(new Uint8Array(response.result.data)) : null;
        report.cases.push({ inputId, format, condition, targetBytes, options, startedAt, completedAt,
          ok: response.ok, error: response.error ?? null, result, output });
        return response;
      }
      try {
        const baseline = await run('none', null);
        if (baseline.ok) {
          const bytes = baseline.result.bytesOut;
          await run('80%', Math.floor(bytes * 80 / 100));
          await run('50%', Math.floor(bytes * 50 / 100));
        } else {
          for (const condition of ['80%', '50%']) {
            report.cases.push({ inputId, format, condition, targetBytes: null,
              options: common, ok: false, error: { message: 'baseline failed; target unavailable' },
              result: null, output: null });
          }
        }
      } finally { worker.terminate(); }
    }
  }
} catch (error) {
  report.error = { message: error.message, stack: error.stack };
}
document.querySelector('#status').textContent = report.error ? report.error.message : 'Finished';
await fetch('/report', { method: 'POST', headers: { 'content-type': 'application/json' },
  body: JSON.stringify(report) });
