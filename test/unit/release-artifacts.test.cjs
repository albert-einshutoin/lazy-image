const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const root = path.resolve(__dirname, '../..');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-release-guard-'));
const artifacts = path.join(temp, 'artifacts');
fs.mkdirSync(artifacts);
let attempt = 0;

function stage() {
  return spawnSync(process.execPath, [path.join(root, 'scripts/stage-release-artifacts.cjs'),
    artifacts, path.join(temp, `candidate-${attempt++}`)], { cwd: root, encoding: 'utf8' });
}

let result = stage();
assert.notEqual(result.status, 0);
assert.match(result.stderr, /missing or unexpected build artifact/);

for (const target of require('../../package.json').napi.targets) {
  fs.mkdirSync(path.join(artifacts, `bindings-${target}`));
}
const first = path.join(artifacts, 'bindings-x86_64-apple-darwin');
fs.writeFileSync(path.join(first, 'lazy-image.darwin-x64.node'), 'invalid binary');
fs.writeFileSync(path.join(first, 'old.node'), 'stale binary');
result = stage();
assert.notEqual(result.status, 0);
assert.match(result.stderr, /missing or multiple binary candidates/);

fs.unlinkSync(path.join(first, 'old.node'));
if (process.platform !== 'win32') {
  result = stage();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /wrong binary CPU or format/);
}

console.log('release artifact guard rejects missing, multiple, and invalid binary artifacts');
