const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const repository = path.resolve(__dirname, '../../..');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'decode-preview-check-'));
try {
  const preview = path.join(temp, 'preview');
  const run = spawnSync(process.execPath, [path.join(__dirname, 'prepare.cjs'), repository, preview], { encoding: 'utf8' });
  assert.equal(run.status, 0, run.stderr);
  for (const name of ['quality.html', 'quality-fresh.html']) assert.deepEqual(fs.readFileSync(path.join(preview, name)), fs.readFileSync(path.join(repository, 'docs/history/blog-adoption-1.5.0-20261013/preview', name)));
  for (const name of ['diagnostic-quality.html', 'diagnostic-quality-fresh.html', 'control-broken.html', 'control-missing.html', 'control-invalid.html']) {
    const html = fs.readFileSync(path.join(preview, name), 'utf8');
    assert.ok(html.indexOf('<script src="diagnose.js">') < html.indexOf('<img '), `${name} must install listeners before images`);
    assert.equal((html.match(/<script src="diagnose.js">/g) || []).length, 1);
  }
  assert.equal(spawnSync(process.execPath, [path.join(__dirname, 'prepare.cjs'), repository, preview], { encoding: 'utf8' }).status, 1);
  console.log(JSON.stringify({ checks: ['old-html-byte-preservation', 'listener-before-images-five-pages', 'existing-preview-refused'], passed: 3 }));
} finally { fs.rmSync(temp, { recursive: true, force: true }); }
