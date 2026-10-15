const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const { spawnSync, spawn } = require('node:child_process');
const { once } = require('node:events');
const repository = path.resolve(__dirname, '../../..');
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'diagram-transfer-'));
const preview = path.join(temporary, 'preview');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
let server;
(async () => {
  const prepare = file => spawnSync(process.execPath, [file, repository, preview], { encoding: 'utf8' });
  const first = prepare(path.join(__dirname, 'prepare.cjs'));
  assert.equal(first.status, 0, first.stderr);
  const catalogBytes = fs.readFileSync(path.join(preview, 'catalog.json'));
  const catalog = JSON.parse(catalogBytes);
  assert.ok(!fs.readFileSync(path.join(preview, 'idle.html'), 'utf8').includes('<img'));
  const cold = catalog.cases.filter(c => c.phase === 'formal' && c.cache === 'cold');
  assert.equal(cold.length, 24);
  assert.equal(new Set(cold.flatMap(c => c.candidates.map(a => a.url))).size, 72);
  for (const condition of ['d1', 'd2', 'm1', 'm2']) assert.equal(cold.filter(c => c.condition === condition).length, 6);
  assert.equal(catalog.cases.filter(c => c.phase === 'pilot').length, 4);
  for (const trial of [1, 2, 3]) {
    const rows = catalog.cases.filter(c => c.phase === 'formal' && c.trial === trial && c.cache !== 'cold');
    assert.equal(rows.length, 2);
    assert.deepEqual(rows[0].candidates, rows[1].candidates);
    assert.equal(rows[0].cache, 'prime'); assert.equal(rows[1].cache, 'warm');
  }
  for (const row of catalog.cases) {
    const html = fs.readFileSync(path.join(preview, catalog.routes[row.page].file), 'utf8');
    assert.ok(html.indexOf('src="/assets/diagnose.js"') < html.indexOf('<img '));
    assert.ok(!html.includes('data-probe-sha') && !html.includes('rel="preload"'));
    if (row.mode === 'responsive') {
      for (const candidate of row.candidates) assert.ok(html.includes(`${candidate.url} ${candidate.width}w`));
      assert.ok(html.includes(`sizes="${catalog.sizes}"`));
    } else assert.ok(!html.includes('srcset='));
  }
  for (const artifact of catalog.artifacts) {
    assert.equal(hash(fs.readFileSync(path.join(preview, 'assets', artifact.path))), artifact.sha256);
  }
  assert.equal(prepare(path.join(__dirname, 'prepare.cjs')).status, 1);
  assert.deepEqual(fs.readFileSync(path.join(preview, 'catalog.json')), catalogBytes);
  const bad = path.join(temporary, 'tampered-helper'); fs.mkdirSync(bad);
  for (const name of ['prepare.cjs', 'observe.js', 'source-manifest.json']) fs.copyFileSync(path.join(__dirname, name), path.join(bad, name));
  const frozenFile = path.join(bad, 'source-manifest.json');
  const frozen = JSON.parse(fs.readFileSync(frozenFile)); frozen.files[0].sha256 = '0'.repeat(64);
  fs.writeFileSync(frozenFile, JSON.stringify(frozen));
  const rejectedOut = path.join(temporary, 'rejected');
  const reject = spawnSync(process.execPath, [path.join(bad, 'prepare.cjs'), repository, rejectedOut], { encoding: 'utf8' });
  assert.equal(reject.status, 1); assert.equal(fs.existsSync(rejectedOut), false);

  server = spawn(process.execPath, [path.join(__dirname, 'server.cjs'), preview, '0'], { stdio: ['ignore', 'pipe', 'pipe'] });
  let stdout = '', stderr = '';
  server.stdout.on('data', b => { stdout += b; }); server.stderr.on('data', b => { stderr += b; });
  const deadline = Date.now() + 5000;
  while (!stdout.includes('\n') && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 20));
  const listening = JSON.parse(stdout.split('\n')[0]);
  assert.equal(listening.event, 'listening'); assert.equal(listening.host, '127.0.0.1');
  const origin = `http://127.0.0.1:${listening.port}`;
  const target = cold[0].candidates[0];
  const response = await fetch(origin + target.url); const body = Buffer.from(await response.arrayBuffer());
  assert.equal(response.status, 200); assert.equal(hash(body), target.sha256);
  assert.equal(response.headers.get('cache-control'), 'public, max-age=3600, immutable');
  assert.equal(response.headers.get('content-type'), 'image/webp');
  assert.equal(Number(response.headers.get('content-length')), target.bytes);
  const revalidated = await fetch(origin + target.url, { headers: { 'If-None-Match': response.headers.get('etag') } });
  assert.equal(revalidated.status, 304); assert.equal((await revalidated.arrayBuffer()).byteLength, 0);
  assert.equal(revalidated.headers.get('content-length'), null);
  const missing = await fetch(origin + '/not-in-catalog'); await missing.arrayBuffer(); assert.equal(missing.status, 404);
  const invalidMethod = await fetch(origin + target.url, { method: 'POST' }); await invalidMethod.arrayBuffer(); assert.equal(invalidMethod.status, 405);
  const page = await fetch(origin + cold[0].page); await page.arrayBuffer();
  assert.equal(page.headers.get('cache-control'), 'no-store');
  fs.writeFileSync(path.join(preview, 'assets', target.path), 'changed disposable copy');
  const changed = await fetch(origin + target.url); await changed.arrayBuffer(); assert.equal(changed.status, 500);
  const exited = once(server, 'close'); server.kill('SIGTERM'); const [exitCode] = await exited;
  assert.equal(exitCode, 0, stderr);
  const requests = stdout.trim().split('\n').slice(1).map(line => JSON.parse(line));
  const imageRequest = requests.find(r => r.url === target.url);
  assert.equal(imageRequest.responseCompleted, true); assert.equal(imageRequest.bodyBytes, target.bytes);
  assert.equal(imageRequest.sha256, target.sha256); assert.equal(imageRequest.trialGroup, cold[0].group);
  const revalidation = requests.find(r => r.status === 304);
  assert.equal(revalidation.bodyBytes, 0); assert.equal(revalidation.representation.sha256, target.sha256);
  console.log(JSON.stringify({ passed: 8, checks: ['24-cold-unique-asset-urls', '3-prime-warm-pairs', '4-pilot-pages', 'real-manifest-srcset-and-early-observer', 'unchanged-image-bytes', 'existing-output-preserved', 'bad-source-rejected-before-output', 'owned-loopback-response-and-log'] }));
})().catch(error => { console.error(error); process.exitCode = 1; }).finally(() => {
  if (server && server.exitCode === null) server.kill('SIGTERM');
  fs.rmSync(temporary, { recursive: true, force: true });
});
