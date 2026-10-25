const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '..');
const noticePath = path.join(root, 'THIRD_PARTY_NOTICES.txt');
const manifestPath = path.join(root, 'native-manifest.json');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const read = name => fs.readFileSync(path.join(root, name));
const legalName = /^(?:license|copying|copyright|notice|patents)(?:[._-]|$)/i;

function legalFiles(dir, recursive = false, prefix = '') {
  return fs.readdirSync(path.join(dir, prefix), { withFileTypes: true }).sort((a, b) => a.name.localeCompare(b.name, 'en')).flatMap(entry => {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.isDirectory()) return recursive ? legalFiles(dir, true, rel) : [];
    return entry.isFile() && legalName.test(entry.name) ? [rel] : [];
  });
}

function registryChecksums() {
  const result = new Map();
  for (const block of read('Cargo.lock').toString().split(/^\[\[package\]\]\s*$/m).slice(1)) {
    const field = name => new RegExp(`^${name} = "([^"\\r\\n]+)"$`, 'm').exec(block)?.[1];
    const source = field('source');
    if (!source) continue;
    assert.ok(source.startsWith('registry+'), `Unsupported dependency source: ${source}`);
    const checksum = field('checksum');
    assert.match(checksum || '', /^[a-f0-9]{64}$/, 'registry crate checksum missing from Cargo.lock');
    const key = `${field('name')} ${field('version')} ${source}`;
    assert.ok(!result.has(key), `Duplicate registry dependency: ${key}`);
    result.set(key, checksum);
  }
  assert.ok(result.size, 'Cargo.lock contains no registry checksums');
  return result;
}

function sourceTreeHash(directory) {
  const digest = createHash('sha256');
  function visit(prefix) {
    for (const entry of fs.readdirSync(path.join(root, directory, prefix), { withFileTypes: true }).sort((a, b) => a.name < b.name ? -1 : 1)) {
      const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
      if (entry.isDirectory()) visit(rel);
      else {
        assert.ok(entry.isFile(), `Unsupported vendored source entry: ${rel}`);
        digest.update(rel).update('\0').update(hash(fs.readFileSync(path.join(root, directory, rel)))).update('\0');
      }
    }
  }
  visit('');
  return digest.digest('hex');
}

function verifyNoticeBinding(manifest = JSON.parse(fs.readFileSync(manifestPath))) {
  for (const input of manifest.inputs) assert.equal(hash(read(input.path)), input.sha256, `notice input changed: ${input.path}`);
  assert.equal(hash(fs.readFileSync(noticePath)), manifest.noticeSha256, 'native notices differ from manifest');
  assert.equal(manifest.toolchain.version, '1.88.0');
  assert.equal(hash(read('RUST_STD_NOTICES.html')), manifest.toolchain.sha256, 'Rust standard-library notices differ from source');
  assert.ok(manifest.packages.some(pkg => pkg.name === 'libavif-sys' && pkg.files.some(file => file.path === 'libavif/LICENSE')),
    'libavif binary license is missing');
  const checksums = registryChecksums();
  for (const pkg of manifest.packages) {
    if (pkg.source?.startsWith('registry+')) {
      const checksum = checksums.get(`${pkg.name} ${pkg.version} ${pkg.source}`);
      assert.ok(checksum, `registry crate checksum absent from Cargo.lock: ${pkg.name}`);
      assert.equal(pkg.crateChecksum, checksum, `registry crate checksum mismatch: ${pkg.name}`);
    } else {
      assert.equal(pkg.name, 'libavif-sys', 'Only the explicitly vendored libavif-sys is a path dependency');
      assert.equal(pkg.source, null, 'path dependency source must be explicitly null');
      assert.equal(pkg.crateChecksum, null, 'path dependency cannot have a registry checksum');
      const version = /^version = "([^"\r\n]+)"$/m.exec(read('vendor/libavif-sys/Cargo.toml').toString())?.[1];
      assert.ok(version, 'vendored dependency version missing');
      assert.equal(pkg.version, version, 'vendored dependency version mismatch');
      assert.deepEqual(pkg.sourceTree, { path: 'vendor/libavif-sys', sha256: sourceTreeHash('vendor/libavif-sys') }, 'vendored source tree mismatch');
    }
  }
  return manifest;
}

function generate() {
  const cargo = args => execFileSync('cargo', args, { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024 });
  const metadata = JSON.parse(cargo(['metadata', '--locked', '--format-version', '1']));
  const checksums = registryChecksums();
  const targets = require('../package.json').napi.targets;
  const selected = new Set();
  for (const target of targets) {
    const tree = cargo(['tree', '--locked', '--edges', 'normal,build', '--prefix', 'none', '--format', '{p}', '--target', target]);
    for (const line of tree.split('\n')) {
      const match = /^(\S+) v(\S+)/.exec(line);
      if (match) selected.add(`${match[1]} ${match[2]}`);
    }
  }
  const upstreamDir = 'vendor/third-party-licenses';
  const upstream = JSON.parse(read(`${upstreamDir}/sources.json`));
  const toolchain = upstream.find(entry => entry.kind === 'toolchain' && entry.name === 'rust-standard-library' && entry.version === '1.88.0');
  assert.ok(toolchain, 'Rust 1.88 standard-library notices are missing');
  const toolchainBytes = read(`${upstreamDir}/${toolchain.file}`);
  assert.equal(hash(toolchainBytes), toolchain.sha256);
  const inputs = ['Cargo.toml', 'Cargo.lock', 'vendor/libavif-sys/Cargo.toml', 'vendor/libavif-sys/libavif/LICENSE', `${upstreamDir}/sources.json`,
    ...upstream.map(entry => `${upstreamDir}/${entry.file}`)].sort().map(name => ({ path: name, sha256: hash(read(name)) }));
  let notice = 'Third-party notices for lazy-image native distributions\n\n' +
    'Conservative union of normal and build dependencies for the six supported targets,\n' +
    'excluding dev-only dependencies. Build tools and proc macros may not be linked.\n' +
    'Bundled C-library legal files are included separately from their Rust wrappers.\n' +
    'Versions, crate checksums and license-file hashes are recorded in native-manifest.json.\n' +
    'Rust 1.88 standard-library and bundled-component legal text is in RUST_STD_NOTICES.html.\n';
  const packages = [];
  for (const pkg of metadata.packages.filter(pkg => pkg.name !== 'lazy-image' && selected.has(`${pkg.name} ${pkg.version}`))
    .sort((a, b) => `${a.name} ${a.version}`.localeCompare(`${b.name} ${b.version}`, 'en'))) {
    const dir = path.dirname(pkg.manifest_path);
    const recursive = ['libavif-sys', 'libwebp-sys', 'mozjpeg-sys', 'tikv-jemalloc-sys', 'libdeflate-sys', 'libfuzzer-sys'].includes(pkg.name);
    const files = legalFiles(dir, recursive).map(name => ({ path: name, bytes: fs.readFileSync(path.join(dir, name)) }));
    const extra = upstream.find(entry => entry.name === pkg.name && entry.version === pkg.version);
    if (extra) {
      const bytes = read(`${upstreamDir}/${extra.file}`);
      assert.equal(hash(bytes), extra.sha256, `upstream license changed: ${pkg.name}`);
      files.push({ path: `upstream/${extra.file}`, bytes });
    }
    const procMacroOnly = pkg.targets.some(target => target.kind.includes('proc-macro')) &&
      !pkg.targets.some(target => target.kind.includes('lib'));
    assert.ok(files.length || procMacroOnly, `No license text found for ${pkg.name} ${pkg.version}`);
    notice += `\n\n===== ${pkg.name} ${pkg.version} | ${pkg.license} =====\nRepository: ${pkg.repository || pkg.homepage || 'see crate manifest'}\nAuthors: ${pkg.authors.join(', ')}\n`;
    if (!files.length) notice += 'Build-time proc macro only; upstream package supplies an SPDX declaration without a separate license file. No proc-macro source is redistributed.\n';
    for (const file of files) notice += `\n--- ${file.path} ---\n${file.bytes.toString('utf8').trimEnd()}\n`;
    const crateChecksum = pkg.source?.startsWith('registry+') ? checksums.get(`${pkg.name} ${pkg.version} ${pkg.source}`) : null;
    if (pkg.source) assert.ok(crateChecksum, `registry crate checksum missing: ${pkg.name}`);
    else assert.equal(pkg.name, 'libavif-sys', 'Unexpected path dependency');
    packages.push({ name: pkg.name, version: pkg.version, license: pkg.license, source: pkg.source,
      crateChecksum, ...(pkg.source ? {} : { sourceTree: { path: 'vendor/libavif-sys', sha256: sourceTreeHash('vendor/libavif-sys') } }),
      procMacroOnly, files: files.map(file => ({ path: file.path, sha256: hash(file.bytes) })) });
  }
  const manifest = `${JSON.stringify({ scope: 'six-target normal/build dependency union; no dev-only packages', targets, inputs,
    noticeSha256: hash(notice), toolchain, packages }, null, 2)}\n`;
  if (process.argv.includes('--check')) {
    assert.equal(fs.readFileSync(noticePath, 'utf8'), notice, 'Regenerate native notices for the current dependency graph');
    assert.equal(fs.readFileSync(manifestPath, 'utf8'), manifest, 'Regenerate native license manifest');
    assert.ok(read('RUST_STD_NOTICES.html').equals(toolchainBytes), 'Regenerate Rust standard-library notices');
  } else {
    fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
    fs.writeFileSync(noticePath, notice);
    fs.writeFileSync(manifestPath, manifest);
    fs.writeFileSync(path.join(root, 'RUST_STD_NOTICES.html'), toolchainBytes);
  }
  verifyNoticeBinding();
  console.log(`Native license notices verified: ${packages.length} normal/build packages across ${targets.length} targets`);
}

if (require.main === module) generate();
module.exports = { verifyNoticeBinding };
