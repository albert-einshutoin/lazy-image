const cases = await fetch('/config.json').then((response) => response.json());
const report = { userAgent: navigator.userAgent, browserVersion: null, cases: [], setup: [], error: null };
try {
  if (navigator.userAgentData?.getHighEntropyValues) {
    report.browserVersion = (await navigator.userAgentData.getHighEntropyValues(['fullVersionList']))
      .fullVersionList.find((entry) => entry.brand === 'Google Chrome')?.version ?? null;
  }

  for (const item of cases) {
    const inputResponse = await fetch(`/input/${item.id}`);
    if (!inputResponse.ok) throw new Error(`Input ${item.id}: HTTP ${inputResponse.status}`);
    const input = await inputResponse.arrayBuffer();
    const started = performance.now();
    const worker = new Worker('/worker.js', { type: 'module' });
    let nextId = 0;
    const pending = new Map();
    worker.addEventListener('message', (event) => {
      if (event.data?.type === 'setup') {
        report.setup.push({ case: item.id, ...event.data });
        return;
      }
      const waiter = pending.get(event.data?.id);
      if (waiter) {
        pending.delete(event.data.id);
        waiter(event.data);
      }
    });
    worker.addEventListener('error', (event) => {
      for (const waiter of pending.values()) waiter({ ok: false, error: { message: event.message } });
      pending.clear();
    });
    const call = async (options) => {
      const id = ++nextId;
      const start = performance.now();
      let timeout;
      const response = await Promise.race([
        new Promise((resolve) => {
          pending.set(id, resolve);
          worker.postMessage({ id, input, options });
        }),
        new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error(`${item.id} Worker timeout`)), 180000); }),
      ]).finally(() => clearTimeout(timeout));
      const wallMs = performance.now() - start;
      return { response, wallMs };
    };
    const cold = await call(item.options);
    const coldFromBeforeWorkerMs = performance.now() - started;
    const warm = [];
    for (let index = 0; index < 2; index++) warm.push(await call(item.options));
    let bestEffort;
    let strict;
    if (item.budgetCases) {
      bestEffort = await call({ ...item.options, targetBytes: 10, minQuality: 50,
        maxQuality: 51, qualityFloorPolicy: 'best-effort' });
      strict = await call({ ...item.options, targetBytes: 10, minQuality: 50,
        maxQuality: 51, qualityFloorPolicy: 'strict' });
    }
    const compact = ({ response, wallMs }, withData = false) => ({
      wallMs, ok: response.ok, error: response.error ?? null,
      result: response.ok ? {
        ...response.result,
        data: withData ? Array.from(new Uint8Array(response.result.data)) : undefined,
      } : null,
    });
    report.cases.push({ id: item.id, coldFromBeforeWorkerMs,
      cold: compact(cold, true), warm: warm.map((run) => compact(run)),
      bestEffort: bestEffort ? compact(bestEffort, true) : null,
      strict: strict ? compact(strict) : null });
    worker.terminate();
  }
} catch (error) {
  report.error = { message: error.message, stack: error.stack };
}
document.querySelector('#status').textContent = report.error ? `FAIL: ${report.error.message}` : 'Finished';
await fetch('/report', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(report) });
