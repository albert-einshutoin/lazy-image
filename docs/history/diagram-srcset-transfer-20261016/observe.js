// #906 performs individual decode and early event capture. No probe attributes are supplied.
const observationStarted = { at: new Date().toISOString(), page: location.href,
  viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio } };
document.addEventListener('DOMContentLoaded', () => {
  const source = document.querySelector('#decode-observation');
  const target = document.querySelector('#responsive-observation');
  const config = JSON.parse(document.querySelector('#case-config').textContent);
  let finished = false;
  const observer = new MutationObserver(() => capture(false));
  const timer = setTimeout(() => capture(true), 30000);
  observer.observe(source, { childList: true, subtree: true, characterData: true });
  async function capture(timedOut) {
    if (finished || (!timedOut && source.textContent === 'loading')) return;
    finished = true; clearTimeout(timer); observer.disconnect();
    let qa = null, observerError = null;
    if (source.textContent !== 'loading') {
      try { qa = JSON.parse(source.textContent); }
      catch (error) { observerError = { name: error.name, message: error.message }; }
    }
    let serviceWorker = { supported: 'serviceWorker' in navigator, controller: null, registrations: null, error: null };
    if (serviceWorker.supported) {
      serviceWorker.controller = navigator.serviceWorker.controller?.scriptURL || null;
      let deadline;
      try {
        const rows = await Promise.race([navigator.serviceWorker.getRegistrations(),
          new Promise((_, reject) => { deadline = setTimeout(() => reject(new Error('service-worker-observation-timeout')), 5000); })]);
        serviceWorker.registrations = rows.map(r => ({ scope: r.scope, active: r.active?.scriptURL || null }));
      } catch (error) { serviceWorker.error = { name: error.name, message: error.message }; }
      finally { clearTimeout(deadline); }
    }
    const keys = ['name', 'initiatorType', 'startTime', 'requestStart', 'responseStart', 'responseEnd', 'duration', 'transferSize',
      'encodedBodySize', 'decodedBodySize', 'responseStatus', 'deliveryType', 'nextHopProtocol', 'workerStart'];
    const resources = performance.getEntriesByType('resource').map(r => Object.fromEntries(keys.map(key => [key, key in r ? r[key] : null])));
    const candidateUrls = new Set(config.candidates.map(a => new URL(a.url, location.href).href));
    // Keep duplicate requests: fetching two variants or retrying also consumes bytes.
    const imageRequests = resources.filter(r => candidateUrls.has(r.name));
    target.textContent = JSON.stringify({ caseId: config.id, started: observationStarted,
      completed: { at: new Date().toISOString(), page: location.href, viewport: { width: innerWidth, height: innerHeight, dpr: devicePixelRatio } },
      pending: timedOut && !qa, observerError, visualViewportScale: visualViewport?.scale ?? null,
      serviceWorker, qa, resources, imageRequests,
      measurementScope: 'resources contains all entries; imageRequests contains candidate URLs including duplicates, without scripts; no diagnostic fetch or client body hash; Resource Timing is not packet capture' }, null, 2);
  }
  capture(false);
});
