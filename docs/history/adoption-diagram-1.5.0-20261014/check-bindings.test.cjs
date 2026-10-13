// Mutate only a disposable copy to prove the saved-evidence checks reject misbinding.
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const [repository, history, originalPhase] = process.argv.slice(2);
if (!repository || !history || !originalPhase) throw Error('Usage: node check-bindings.test.cjs <repository> <saved-history> <original-phase>');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'diagram-binding-check-'));
const copy = path.join(temp, 'history');
const results = [];
try {
  fs.cpSync(history, copy, { recursive: true });
  const check = kind => spawnSync(process.execPath, kind === 'browser'
    ? [path.join(history, 'check-browser.cjs'), repository, copy, originalPhase, '--check']
    : [path.join(history, 'check-outputs.cjs'), repository, path.join(copy, 'evidence'), '--check'], { encoding: 'utf8' });
  for (const kind of ['outputs', 'browser']) {
    const result = check(kind); assert.equal(result.status, 0, result.stderr);
    results.push({ case: `valid-${kind}`, exit: result.status });
  }
  const change = (name, kind, file, mutate) => {
    const target = path.join(copy, file), original = fs.readFileSync(target);
    try {
      const data = JSON.parse(original); mutate(data);
      fs.writeFileSync(target, JSON.stringify(data, null, 2) + '\n');
      const result = check(kind); assert.equal(result.error, undefined);
      assert.equal(result.status, 1, `${name} unexpectedly passed`);
      results.push({ case: name, exit: result.status });
    } finally { fs.writeFileSync(target, original); }
  };
  const capture = 'evidence/browser/mobile-1-detail-svg-verify-middle.json';
  change('duplicate-pose', 'browser', capture, d => { d.pose = 'verify-left'; });
  change('wrong-pan', 'browser', capture, d => { d.before.pan.left = 0; });
  change('duplicate-desktop-part', 'browser', 'evidence/browser/desktop-1-svg-bottom.json', d => { d.part = 'top'; });
  const descriptor = 'evidence/browser/captures/mobile-1-detail-svg-verify-middle.png.json';
  change('wrong-original-capture', 'browser', descriptor, d => { d.originalSha256 = '0'.repeat(64); });
  change('wrong-capture-record', 'browser', descriptor, d => { d.captureRecordSha256 = '0'.repeat(64); });
  for (const [name, mutate] of [
    ['wrong-helper-revision', d => { d[0].evaluationRevision = '0'.repeat(40); }],
    ['wrong-helper-hash', d => { d[0].codeHashes[0].sha256 = '0'.repeat(64); }],
    ['wrong-command', d => { d[0].command[1] = '<OTHER_CONSUMER>/compile.mjs'; }],
    ['spawn-failure', d => { d[0].spawnError = 'failed'; }],
    ['missing-output-inventory', d => { d[0].files.pop(); }],
  ]) change(name, 'outputs', 'evidence/runs/processes.json', mutate);
  for (const [name, mutate] of [
    ['wrong-input', d => { d.inputSha256 = '0'.repeat(64); }],
    ['wrong-policy', d => { d.policySha256 = '0'.repeat(64); }],
    ['wrong-source', d => { d.identity.sourceSha = '0'.repeat(40); }],
    ['wrong-native', d => { d.identity.nativeSha256 = '0'.repeat(64); }],
    ['wrong-lock', d => { d.identity.lockSha256 = '0'.repeat(64); }],
  ]) change(name, 'outputs', 'evidence/runs/A1.json', mutate);
  console.log(JSON.stringify({ scope: 'Disposable metadata tampering only; no image generation or original mutation', results }, null, 2));
} finally { fs.rmSync(temp, { recursive: true, force: true }); }
