const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const sourceCode = fs.readFileSync(path.join(__dirname, 'observe.js'), 'utf8');
async function observe(qa, swError = false) {
  const source = { textContent: 'loading' }, target = { textContent: 'loading' };
  let ready, mutation;
  const context = vm.createContext({ document: {
    addEventListener(name, fn) { if (name === 'DOMContentLoaded') ready = fn; },
    querySelector(selector) { return selector === '#decode-observation' ? source : selector === '#responsive-observation' ? target : { textContent: '{"id":"unit-fixture","candidates":[{"url":"/image.webp"}]}' }; },
  }, MutationObserver: class { constructor(fn) { mutation = fn; } observe() {} disconnect() {} },
  location: { href: 'http://localhost/unit-fixture.html' }, innerWidth: 390, innerHeight: 844, devicePixelRatio: 2,
  visualViewport: { scale: 1 }, navigator: { serviceWorker: { controller: null, getRegistrations: () => swError ? Promise.reject(new Error('observation denied')) : Promise.resolve([]) } },
  performance: { getEntriesByType: () => [{ name: 'http://localhost/image.webp', responseStatus: 0, transferSize: 0 },
    { name: 'http://localhost/image.webp', responseStatus: 200, transferSize: 1234 },
    { name: 'http://localhost/assets/diagnose.js', initiatorType: 'script', transferSize: 9999 }] },
  Date, JSON, Promise, URL, setTimeout: (fn, ms) => setTimeout(fn, Math.min(ms, 25)), clearTimeout });
  vm.runInContext(sourceCode, context); ready();
  if (qa) { source.textContent = JSON.stringify(qa); mutation(); }
  const until = Date.now() + 500;
  while (target.textContent === 'loading' && Date.now() < until) await new Promise(resolve => setTimeout(resolve, 5));
  return JSON.parse(target.textContent);
}
(async () => {
  const qa = { images: ['success', 'error', 'timeout', 'unknown'].map(status => ({ id: status, status })) };
  const good = await observe(qa);
  assert.deepEqual(good.qa, qa); assert.equal(good.pending, false);
  assert.equal(good.resources[0].responseStatus, 0); assert.equal(good.resources[0].transferSize, 0);
  assert.equal(good.resources[0].encodedBodySize, null);
  assert.equal(good.resources.length, 3); assert.equal(good.imageRequests.length, 2);
  assert.deepEqual(good.imageRequests.map(r => r.name), ['http://localhost/image.webp', 'http://localhost/image.webp']);
  assert.equal(good.imageRequests.reduce((sum, r) => sum + r.transferSize, 0), 1234);
  assert.deepEqual(good.serviceWorker.registrations, []);
  assert.ok(good.started.at && good.completed.at);
  const pending = await observe(null); assert.equal(pending.pending, true); assert.equal(pending.qa, null);
  const denied = await observe(qa, true); assert.equal(denied.serviceWorker.registrations, null); assert.equal(denied.serviceWorker.error.message, 'observation denied');
  console.log(JSON.stringify({ passed: 5, checks: ['individual-statuses-preserved', 'zero-and-unsupported-timing-distinct', 'candidate-transfers-exclude-script-and-retain-duplicates', 'unfinished-decode-remains-pending', 'service-worker-failure-remains-unknown'] }));
})().catch(error => { console.error(error); process.exitCode = 1; });
