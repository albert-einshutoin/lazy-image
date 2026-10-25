const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');

const root = path.resolve(__dirname, '../..');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-native-loader-'));
try {
  const consumer = path.join(temp, 'consumer');
  fs.mkdirSync(consumer);
  for (const name of ['index.js', 'package.json', 'lib', 'streaming']) {
    fs.cpSync(path.join(root, name), path.join(consumer, name), { recursive: true });
  }
  const platform = process.platform === 'linux'
    ? `${process.platform}-${process.arch}-${process.report.getReport().header.glibcVersionRuntime ? 'gnu' : 'musl'}`
    : process.platform === 'win32' ? 'win32-x64-msvc' : `${process.platform}-${process.arch}`;
  const packageDir = path.join(consumer, 'node_modules', `@alberteinshutoin/lazy-image-${platform}`);
  fs.mkdirSync(packageDir, { recursive: true });
  const evaluated = path.join(temp, 'evaluated');
  fs.writeFileSync(path.join(packageDir, 'index.js'), `require('node:fs').writeFileSync(${JSON.stringify(evaluated)}, 'yes'); throw new Error('addon evaluated');`);
  function run(extra = {}) {
    const env = { ...process.env, ...extra };
    for (const name of ['NODE_PATH', 'NAPI_RS_NATIVE_LIBRARY_PATH', 'NAPI_RS_FORCE_WASI', 'NAPI_RS_WASI_FLAVOR']) delete env[name];
    return spawnSync(process.execPath, ['-e', `require(${JSON.stringify(consumer)})`], { env, encoding: 'utf8' });
  }
  for (const metadata of [{ version: '1.5.0' }, {}, { version: 2 }, 'malformed', null]) {
    const file = path.join(packageDir, 'package.json');
    if (metadata === null) fs.rmSync(file, { force: true });
    else fs.writeFileSync(file, metadata === 'malformed' ? '{invalid' : JSON.stringify({ main: 'index.js', ...metadata }));
    for (const enforce of [undefined, '0', '1']) {
      const result = run({ NAPI_RS_ENFORCE_VERSION_CHECK: enforce });
      assert.notEqual(result.status, 0);
      assert.equal(fs.existsSync(evaluated), false, `native code evaluated for ${JSON.stringify(metadata)}, enforce=${enforce}`);
      assert.match(result.stderr, /version|metadata/i);
    }
  }
  const local = path.join(consumer, `lazy-image.${platform}.node`);
  fs.writeFileSync(local, 'not a native module');
  for (const metadata of [null, { version: '1.5.0', sha256: 'invalid' }, { version: '2.0.0', sha256: 'invalid' }]) {
    if (metadata) fs.writeFileSync(`${local}.json`, JSON.stringify(metadata));
    const result = run();
    assert.notEqual(result.status, 0);
    assert.equal(fs.existsSync(evaluated), false, 'invalid local metadata must not fall through to a package');
  }
  console.log('native loader rejects stale, missing and malformed metadata before addon evaluation');
} finally {
  fs.rmSync(temp, { recursive: true, force: true });
}
