// Diagnostic only: observe every decode independently; no rendering fallback.
const decodeEvents = [];
for (const type of ['load', 'error']) document.addEventListener(type, event => {
  if (event.target.tagName === 'IMG') decodeEvents.push({ type, id: event.target.alt, src: event.target.currentSrc || event.target.src, atMs: performance.now() });
}, true);
const viewport = () => ({ width: innerWidth, height: innerHeight, dpr: devicePixelRatio });
const snapshot = image => ({ src: image.getAttribute('src'), currentSrc: image.currentSrc || image.src,
  srcset: image.getAttribute('srcset'), sizes: image.sizes || '', complete: image.complete,
  naturalWidth: image.naturalWidth, naturalHeight: image.naturalHeight,
  rect: { width: image.getBoundingClientRect().width, height: image.getBoundingClientRect().height } });
async function inspectDecodedImage(image, timeoutMs = 10000) {
  const before = snapshot(image); let timer;
  const result = await Promise.race([
    Promise.resolve().then(() => image.decode()).then(() => ({ status: 'success', error: null }),
      error => ({ status: 'error', error: { name: error?.name || 'ThrownValue', message: error?.message || String(error) } })),
    new Promise(resolve => { timer = setTimeout(() => resolve({ status: 'timeout', error: null }), timeoutMs); }),
  ]);
  clearTimeout(timer); const after = snapshot(image);
  const changed = ['src', 'currentSrc', 'srcset', 'sizes'].some(key => before[key] !== after[key]);
  const reason = changed ? 'source-changed' : result.status === 'success' && (!after.naturalWidth || !after.naturalHeight) ? 'empty-natural-dimensions' : null;
  return { id: image.alt, ...result, status: reason ? 'unknown' : result.status, reason, before, after, timeoutMs };
}
async function probeBody(image) {
  const url = new URL(image.currentSrc || image.src, location.href);
  if (url.origin !== location.origin || !['http:', 'https:'].includes(url.protocol)) return { url: url.href, status: 'rejected-origin' };
  const controller = new AbortController(), timer = setTimeout(() => controller.abort(), 10000);
  try {
    const response = await fetch(url, { signal: controller.signal, cache: 'no-store' });
    const body = await response.arrayBuffer();
    const sha256 = Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256', body)), b => b.toString(16).padStart(2, '0')).join('');
    const headers = Object.fromEntries(['content-type', 'content-length', 'last-modified', 'cache-control', 'etag'].map(name => [name, response.headers.get(name)]));
    const record = { url: url.href, responseUrl: response.url, redirected: response.redirected, httpStatus: response.status,
      headers, bytes: body.byteLength, sha256, expectedSha256: image.dataset.probeSha, hashMatches: sha256 === image.dataset.probeSha,
      scope: 'Independent no-store fetch after DOM decode; not the original img request body' };
    if (!response.ok) return { ...record, status: 'http-error', blobDecode: null };
    const blobUrl = URL.createObjectURL(new Blob([body], { type: headers['content-type'] || '' }));
    try { const candidate = new Image(); candidate.alt = image.alt + ' independent body'; candidate.src = blobUrl;
      return { ...record, status: record.hashMatches ? 'body-matches' : 'body-mismatch', blobDecode: await inspectDecodedImage(candidate) };
    } finally { URL.revokeObjectURL(blobUrl); }
  } catch (error) { return { url: url.href, status: error?.name === 'AbortError' ? 'fetch-timeout' : 'fetch-error', error: { name: error?.name, message: error?.message || String(error) } }; }
  finally { clearTimeout(timer); }
}
addEventListener('load', async () => {
  const urlBefore = location.href, viewportBefore = viewport(), images = [...document.images];
  const results = await Promise.all(images.map(image => inspectDecodedImage(image)));
  const probes = await Promise.all(images.filter(image => image.dataset.probeSha).map(probeBody));
  const browserVersion = navigator.userAgentData ? await navigator.userAgentData.getHighEntropyValues(['fullVersionList']) : null;
  document.querySelector('#decode-observation').textContent = JSON.stringify({ urlBefore, urlAfter: location.href,
    viewportBefore, viewportAfter: viewport(), userAgent: navigator.userAgent, browserVersion,
    images: results, probes, events: decodeEvents,
    resources: performance.getEntriesByType('resource').map(r => ({ name: r.name, type: r.initiatorType,
      responseStatus: r.responseStatus, transferSize: r.transferSize, encodedBodySize: r.encodedBodySize, decodedBodySize: r.decodedBodySize })) }, null, 2);
});
