const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const root = path.resolve(__dirname, '../..');
const { verifyNoticeBinding } = require('../../scripts/native-notices.cjs');
const currentNotices = require('../../native-manifest.json');
verifyNoticeBinding(currentNotices);
for (const checksum of [null, '0'.repeat(64)]) {
  const invalid = structuredClone(currentNotices);
  invalid.packages.find(pkg => pkg.source?.startsWith('registry+')).crateChecksum = checksum;
  assert.throws(() => verifyNoticeBinding(invalid), /registry.*checksum/i,
    'staging must reject absent or incorrect registry crate provenance');
}
for (const changes of [{ source: 'git+https://example.invalid/repository' }, { crateChecksum: '0'.repeat(64) }, { version: '0.0.0' }]) {
  const invalid = structuredClone(currentNotices);
  Object.assign(invalid.packages.find(pkg => pkg.name === 'libavif-sys'), changes);
  assert.throws(() => verifyNoticeBinding(invalid), /path dependency|vendored.*version/i,
    'the explicit path-only exception must reject another source, checksum or version');
}
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-release-guard-'));
const checkout = path.join(temp, 'crlf-checkout');
fs.mkdirSync(checkout);
const vendored = spawnSync('git', ['ls-files', '-z', 'vendor/libavif-sys'], { cwd: root, encoding: 'utf8' });
assert.equal(vendored.status, 0, vendored.stderr);
const boundFiles = [...new Set([
  '.gitattributes', 'THIRD_PARTY_NOTICES.txt', 'RUST_STD_NOTICES.html', 'native-manifest.json',
  ...currentNotices.inputs.map(input => input.path), ...vendored.stdout.split('\0').filter(Boolean),
])];
for (const name of boundFiles) {
  fs.mkdirSync(path.dirname(path.join(checkout, name)), { recursive: true });
  fs.copyFileSync(path.join(root, name), path.join(checkout, name));
}
for (const args of [['init', '--quiet'], ['-c', 'core.autocrlf=false', 'add', '--force', '--all']]) {
  const result = spawnSync('git', args, { cwd: checkout, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
}
for (const name of boundFiles) fs.unlinkSync(path.join(checkout, name));
const restored = spawnSync('git', ['-c', 'core.autocrlf=true', 'checkout-index', '--all'], { cwd: checkout, encoding: 'utf8' });
assert.equal(restored.status, 0, restored.stderr);
for (const name of boundFiles.filter(name => name !== '.gitattributes')) {
  assert.ok(fs.readFileSync(path.join(checkout, name)).equals(fs.readFileSync(path.join(root, name))),
    `CRLF checkout changed byte-bound source: ${name}`);
}
console.log('CRLF checkout preserves all byte-bound notice inputs and vendored sources');

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
fs.writeFileSync(path.join(first, 'lazy-image.darwin-x64.node.json'), '{}');
if (process.platform !== 'win32') {
  result = stage();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /wrong binary CPU or format/);
}

console.log('release artifact guard rejects missing, multiple, and invalid binary artifacts');

const payload = Buffer.alloc(2 * 1024 * 1024, 0x6a);
const packageDir = path.join(temp, 'package');
fs.mkdirSync(packageDir);
fs.writeFileSync(path.join(packageDir, 'large.node'), payload);
const archive = path.join(temp, 'large.tgz');
result = spawnSync('tar', ['-czf', archive, '-C', temp, 'package'], { encoding: 'utf8' });
assert.equal(result.status, 0);
const extracted = require('../../scripts/stage-release-artifacts.cjs').tarEntry(archive, 'package/large.node');
assert.equal(createHash('sha256').update(extracted).digest('hex'),
  createHash('sha256').update(payload).digest('hex'));
console.log('release artifact guard verifies binaries larger than the default child-process buffer');

const { stageWasm, verifyWasmTarball } = require('../../scripts/stage-release-artifacts.cjs');
assert.equal(typeof stageWasm, 'function', 'Wasm must be an actual retained candidate');
const wasmOutput = path.join(temp, 'wasm-candidate');
fs.mkdirSync(wasmOutput);
const wasm = stageWasm(wasmOutput);
assert.equal(wasm.name, '@alberteinshutoin/lazy-image-wasm');
assert.equal(wasm.files.length, 29);
assert.ok(wasm.files.some(file => file.path === 'LICENSE'));
for (const codec of ['dec/webp_dec', 'enc/webp_enc', 'enc/webp_enc_simd']) {
  assert.ok(wasm.files.some(file => file.path === `vendor/webp/codec/${codec}.wasm`),
    `patched WebP codec missing from candidate: ${codec}`);
}
assert.equal(wasm.tarballSha256, createHash('sha256').update(fs.readFileSync(path.join(wasmOutput, wasm.tarball))).digest('hex'));
const altered = path.join(temp, 'altered');
fs.mkdirSync(altered);
result = spawnSync('tar', ['-xzf', path.join(wasmOutput, wasm.tarball), '-C', altered], { encoding: 'utf8' });
assert.equal(result.status, 0);
fs.appendFileSync(path.join(altered, 'package', 'shared.js'), '\n// altered candidate\n');
const alteredTarball = path.join(temp, 'altered-wasm.tgz');
result = spawnSync('tar', ['-czf', alteredTarball, '-C', altered, 'package'], { encoding: 'utf8' });
assert.equal(result.status, 0);
assert.throws(() => verifyWasmTarball(alteredTarball, wasm.files), /Wasm candidate differs from source/);
console.log('Wasm candidate is retained with content hashes and rejects altered payloads');

const legal = path.join(temp, 'legal', 'package');
fs.mkdirSync(legal, { recursive: true });
for (const name of ['LICENSE', 'THIRD_PARTY_NOTICES.txt', 'RUST_STD_NOTICES.html', 'native-manifest.json']) fs.copyFileSync(path.join(root, name), path.join(legal, name));
const legalTar = path.join(temp, 'legal.tgz');
function packLegal() {
  const result = spawnSync('tar', ['-czf', legalTar, '-C', path.dirname(legal), 'package'], { encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
}
packLegal();
const { verifyLegalFiles } = require('../../scripts/stage-release-artifacts.cjs');
assert.equal(verifyLegalFiles(legalTar).length, 4);
fs.appendFileSync(path.join(legal, 'THIRD_PARTY_NOTICES.txt'), 'altered');
packLegal();
assert.throws(() => verifyLegalFiles(legalTar), /legal file differs/);
fs.rmSync(path.join(legal, 'LICENSE'));
packLegal();
assert.throws(() => verifyLegalFiles(legalTar));
console.log('candidate legal content validation rejects altered notices and missing project license');

const { selectPackages } = require('../../scripts/publish-release-candidate.cjs');
const mainName = require('../../package.json').name;
const records = [{ name: mainName }, { name: `${mainName}-wasm` },
  ...Object.keys(require('../../package.json').optionalDependencies).map(name => ({ name, platform: 'target' }))];
assert.deepEqual(selectPackages(records, 'main').map(p => p.name), [mainName]);
assert.deepEqual(selectPackages(records, 'wasm').map(p => p.name), [`${mainName}-wasm`]);
assert.equal(selectPackages(records, 'platform').length, 6);
assert.throws(() => selectPackages(records.filter(p => !p.name.endsWith('-wasm')), 'wasm'));
assert.throws(() => selectPackages([...records, records[1]], 'wasm'));
console.log('candidate publication selects exact groups and rejects missing or duplicate packages');
