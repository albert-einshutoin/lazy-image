const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const source = process.argv[2] || path.join(__dirname, 'diagnose.js');
const images = [
  ['good', () => Promise.resolve()],
  ['broken-1', () => Promise.reject(Object.assign(new Error('bad bytes one'), { name: 'EncodingError' }))],
  ['broken-2', () => Promise.reject(Object.assign(new Error('bad bytes two'), { name: 'EncodingError' }))],
  ['pending', () => new Promise(() => {})],
].map(([id, decode]) => ({ alt: id, src: `http://localhost/${id}.png`, currentSrc: `http://localhost/${id}.png`,
  complete: true, naturalWidth: id === 'good' ? 64 : 0, naturalHeight: id === 'good' ? 43 : 0,
  dataset: {}, decode, getAttribute(name) { return name === 'src' ? this.src : null; },
  getBoundingClientRect() { return { width: 64, height: 43 }; } }));
const output = { textContent: '' }; let onLoad;
const context = vm.createContext({ document: { images, addEventListener() {}, querySelector() { return output; } },
  addEventListener(name, fn) { if (name === 'load') onLoad = fn; },
  location: { href: 'http://localhost/test.html', origin: 'http://localhost' }, innerWidth: 1440, innerHeight: 1000,
  devicePixelRatio: 1, navigator: { userAgent: 'unit fixture' }, getComputedStyle: () => ({ backgroundImage: 'none' }),
  performance: { now: () => Date.now(), getEntriesByType: () => [] },
  // The timeout contract is tested with a short fixture clock, not browser latency.
  setTimeout: (fn, ms) => setTimeout(fn, Math.min(ms, 25)), clearTimeout, URL, console });
(async () => {
  vm.runInContext(fs.readFileSync(source, 'utf8'), context); await onLoad();
  const result = JSON.parse(output.textContent);
  assert.deepEqual(result.images.map(i => [i.id, i.status]), [['good', 'success'], ['broken-1', 'error'], ['broken-2', 'error'], ['pending', 'timeout']]);
  assert.equal(result.images[1].error.message, 'bad bytes one'); assert.equal(result.images[2].error.message, 'bad bytes two');
  const changing = { ...images[0], decode() { this.currentSrc = 'http://localhost/other.png'; return Promise.resolve(); } };
  assert.equal((await context.inspectDecodedImage(changing, 25)).status, 'unknown');
  const late = { ...images[0], decode: () => new Promise(resolve => setTimeout(resolve, 60)) };
  const timed = await context.inspectDecodedImage(late, 25); assert.equal(timed.status, 'timeout');
  await new Promise(resolve => setTimeout(resolve, 70)); assert.equal(timed.status, 'timeout');
  console.log(JSON.stringify({ checks: ['individual-success', 'two-distinct-errors', 'pending-timeout', 'source-change-unknown', 'late-success-does-not-rewrite-timeout'], passed: 5 }));
})().catch(error => { console.error(error); process.exitCode = 1; });
