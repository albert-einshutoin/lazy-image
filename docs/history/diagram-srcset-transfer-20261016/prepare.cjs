const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const [repository, destination] = process.argv.slice(2);
if (!repository || !destination || fs.existsSync(destination)) throw Error('Usage: node prepare.cjs <repository> <new-preview>');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const frozen = JSON.parse(fs.readFileSync(path.join(__dirname, 'source-manifest.json')));
for (const row of frozen.files) {
  const bytes = fs.readFileSync(path.join(repository, row.file));
  assert.equal(bytes.length, row.bytes, row.file); assert.equal(hash(bytes), row.sha256, row.file);
}
const old = path.join(repository, 'docs/history/adoption-diagram-1.5.0-20261014');
const manifest = JSON.parse(fs.readFileSync(path.join(old, 'evidence/runs/B1/manifest.json')));
assert.equal(manifest.compiler.version, '1.5.0');
const artifacts = manifest.artifacts.filter(a => a.format === 'webp').sort((a, b) => a.width - b.width);
assert.deepEqual(artifacts.map(a => a.width), [684, 960, 1920]);
for (const a of artifacts) { assert.equal(path.basename(a.path), a.path); assert.match(a.path, /^[a-z0-9.-]+\.webp$/); }
fs.mkdirSync(destination); fs.mkdirSync(path.join(destination, 'assets')); fs.mkdirSync(path.join(destination, 'pages'));
const routes = {}, cases = [], namespace = crypto.randomUUID();
const sizes = '(max-width: 1008px) calc(100vw - 48px), 960px';
function route(url, file, type, group = null) {
  const body = fs.readFileSync(path.join(destination, file));
  const value = { file, type, group, bytes: body.length, sha256: hash(body) };
  if (routes[url]) assert.deepEqual(routes[url], value); else routes[url] = value;
}
fs.writeFileSync(path.join(destination, 'idle.html'), '<!doctype html><meta name="viewport" content="width=device-width,initial-scale=1"><title>Measurement idle</title><p>No images. Set the next viewport before navigation.</p>\n', { flag: 'wx' });
route('/idle.html', 'idle.html', 'text/html; charset=utf-8');
for (const a of artifacts) {
  const bytes = fs.readFileSync(path.join(old, 'evidence/runs/B1', a.path));
  assert.equal(hash(bytes), a.sha256); assert.equal(bytes.length, a.bytes);
  fs.writeFileSync(path.join(destination, 'assets', a.path), bytes, { flag: 'wx' });
}
fs.copyFileSync(path.join(repository, 'docs/history/decode-comparison-20261014/diagnose.js'), path.join(destination, 'assets/diagnose.js'));
fs.copyFileSync(path.join(__dirname, 'observe.js'), path.join(destination, 'assets/observe.js'));
for (const file of ['diagnose.js', 'observe.js']) route('/assets/' + file, 'assets/' + file, 'text/javascript; charset=utf-8');
const conditions = { d1: { width: 1440, height: 1000, dpr: 1, cssWidth: 960 }, d2: { width: 1440, height: 1000, dpr: 2, cssWidth: 960 }, m1: { width: 390, height: 844, dpr: 1, cssWidth: 342 }, m2: { width: 390, height: 844, dpr: 2, cssWidth: 342 } };
function add(phase, condition, mode, cache, trial, group) {
  const id = `${phase}-${condition}-${mode}-${cache}-${trial}`;
  const prefix = `/${namespace}/${group}/`;
  const candidates = artifacts.map(a => ({ ...a, url: prefix + a.path }));
  for (const a of candidates) route(a.url, 'assets/' + a.path, 'image/webp', group);
  const page = prefix + `${mode}-${cache}.html`;
  const row = { id, phase, condition, mode, cache, trial, group, page, expected: conditions[condition], candidates };
  const largest = candidates.at(-1);
  const responsive = mode === 'responsive' ? ` srcset="${candidates.map(a => `${a.url} ${a.width}w`).join(', ')}" sizes="${sizes}"` : '';
  const html = `<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Diagram transfer: ${id}</title><script id="case-config" type="application/json">${JSON.stringify(row)}</script><script src="/assets/diagnose.js"></script><script src="/assets/observe.js"></script><style>body{margin:0;background:white;color:#172034;font:16px Arial,sans-serif}main{max-width:1008px;box-sizing:border-box;padding:24px;margin:auto}h1{font-size:18px}img{display:block;width:100%;max-width:960px;height:auto}pre{white-space:pre-wrap;font-size:12px}</style><main><h1>${id}</h1><img alt="Compiler flow diagram" src="${largest.url}"${responsive} width="${largest.width}" height="${largest.height}"><details><summary>Individual decode</summary><pre id="decode-observation">loading</pre></details><details open><summary>Transfer observation</summary><pre id="responsive-observation">loading</pre></details></main></html>\n`;
  const file = `pages/${id}.html`; fs.writeFileSync(path.join(destination, file), html, { flag: 'wx' });
  route(page, file, 'text/html; charset=utf-8', group); cases.push(row);
}
add('pilot', 'd1', 'fixed', 'cold', 1, 'pilot-d1-fixed');
add('pilot', 'm2', 'responsive', 'cold', 1, 'pilot-m2-responsive');
add('pilot', 'm2', 'fixed', 'prime', 1, 'pilot-warm-1');
add('pilot', 'm2', 'responsive', 'warm', 1, 'pilot-warm-1');
for (const condition of Object.keys(conditions)) for (const trial of [1, 2, 3]) for (const mode of ['responsive', 'fixed']) {
  add('formal', condition, mode, 'cold', trial, `${condition}-${mode}-${trial}`);
}
for (const trial of [1, 2, 3]) {
  add('formal', 'm2', 'fixed', 'prime', trial, `warm-${trial}`);
  add('formal', 'm2', 'responsive', 'warm', trial, `warm-${trial}`);
}
const catalog = { issue: 908, namespace, scope: 'Stored public npm 1.5.0 outputs; compiler not executed', sizes, compiler: manifest.compiler,
  sourceManifestSha256: hash(fs.readFileSync(path.join(__dirname, 'source-manifest.json'))), artifacts, routes, cases };
fs.writeFileSync(path.join(destination, 'catalog.json'), JSON.stringify(catalog, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ cases: cases.length, coldFormal: 24, warmPairs: 3, pilot: 4, uniqueNamespace: namespace }));
