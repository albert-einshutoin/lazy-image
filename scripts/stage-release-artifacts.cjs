const assert = require('node:assert/strict');
const { execFileSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const artifacts = path.resolve(process.argv[2] || 'artifacts');
const output = path.resolve(process.argv[3] || 'candidate');
const platforms = {
  'x86_64-apple-darwin': ['darwin-x64', /Mach-O 64-bit.*x86_64/],
  'aarch64-apple-darwin': ['darwin-arm64', /Mach-O 64-bit.*arm64/],
  'x86_64-pc-windows-msvc': ['win32-x64-msvc', /PE32\+.*x86-64/],
  'x86_64-unknown-linux-gnu': ['linux-x64-gnu', /ELF 64-bit.*x86-64/],
  'aarch64-unknown-linux-gnu': ['linux-arm64-gnu', /ELF 64-bit.*(ARM aarch64|AArch64)/],
  'x86_64-unknown-linux-musl': ['linux-x64-musl', /ELF 64-bit.*x86-64/],
};
const pkg = require('../package.json');

function sha256(file) { return createHash('sha256').update(fs.readFileSync(file)).digest('hex'); }
function command(executable, args, cwd) {
  return execFileSync(executable, args, { cwd, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 }).trim();
}
function tarEntry(tarball, member) {
  return execFileSync('tar', ['-xOzf', tarball, member], { maxBuffer: 64 * 1024 * 1024 });
}
function inspect(file, target, pattern) {
  const description = command('file', ['-b', file]);
  assert.match(description, pattern, `${target}: wrong binary CPU or format: ${description}`);
  const result = { description };
  if (target.includes('linux')) {
    if (target.endsWith('musl')) {
      const bytes = fs.readFileSync(file);
      assert.ok(!bytes.includes(Buffer.from('GLIBC_')) && !bytes.includes(Buffer.from('ld-linux-')),
        `${target}: glibc ABI or loader dependency`);
    }
    const dynamic = command('readelf', ['-dW', file]);
    const versionInfo = command('readelf', ['-VW', file]);
    result.needed = [...dynamic.matchAll(/\(NEEDED\).*?\[(.*?)\]/g)].map(match => match[1]);
    result.glibcVersions = [...new Set(versionInfo.match(/GLIBC_[0-9.]+/g) || [])];
    if (target.endsWith('musl')) {
      assert.ok(!result.needed.some(name => name === 'libc.so.6' || name.startsWith('ld-linux')),
        `${target}: glibc library dependency`);
      assert.equal(result.glibcVersions.length, 0, `${target}: GLIBC version dependency`);
    } else {
      assert.ok(!result.needed.some(name => name.includes('musl')), `${target}: musl library dependency`);
    }
  }
  return result;
}

function main() {
  assert.deepEqual(pkg.napi.targets.slice().sort(), Object.keys(platforms).sort());
  assert.deepEqual(Object.keys(pkg.optionalDependencies).sort(), Object.values(platforms)
    .map(([platform]) => `${pkg.name}-${platform}`).sort());
  for (const version of Object.values(pkg.optionalDependencies)) assert.equal(version, pkg.version);
  assert.ok(!fs.existsSync(output), `candidate directory already exists: ${output}`);
  assert.ok(!fs.existsSync(path.join(root, 'npm')), 'stale npm platform packages exist');
  const expected = Object.keys(platforms).map(target => `bindings-${target}`).sort();
  assert.deepEqual(fs.readdirSync(artifacts).sort(), expected, 'missing or unexpected build artifact');
  fs.mkdirSync(output);
  const packages = [];
  for (const [target, [platform, pattern]] of Object.entries(platforms)) {
    const dir = path.join(artifacts, `bindings-${target}`);
    const binaryName = `lazy-image.${platform}.node`;
    assert.deepEqual(fs.readdirSync(dir), [binaryName], `${target}: missing or multiple binary candidates`);
    const source = path.join(dir, binaryName);
    const inspection = inspect(source, target, pattern);
    const platformDir = path.join(root, 'npm', platform);
    fs.mkdirSync(platformDir, { recursive: true });
    fs.copyFileSync(source, path.join(platformDir, binaryName));
    const name = `${pkg.name}-${platform}`;
    fs.writeFileSync(path.join(platformDir, 'package.json'), `${JSON.stringify({
      name, version: pkg.version, description: pkg.description, main: binaryName,
      os: [platform.split('-')[0]], cpu: [platform.split('-')[1]], files: [binaryName],
      ...(platform.endsWith('-gnu') ? { libc: ['glibc'] } : platform.endsWith('-musl') ? { libc: ['musl'] } : {}),
      license: pkg.license, publishConfig: { access: 'public' }, repository: pkg.repository,
      engines: pkg.engines,
    }, null, 2)}\n`);
    const [packed] = JSON.parse(command('npm', ['pack', '--ignore-scripts', '--json', '--pack-destination', output], platformDir));
    const tarball = path.join(output, packed.filename);
    assert.equal(sha256(source), createHash('sha256').update(tarEntry(tarball,
      `package/${binaryName}`)).digest('hex'), `${platform}: tarball binary differs from verified artifact`);
    const packedPackage = JSON.parse(tarEntry(tarball, 'package/package.json'));
    assert.equal(packedPackage.name, name);
    assert.equal(packedPackage.version, pkg.version);
    assert.deepEqual(packedPackage.os, [platform.split('-')[0]]);
    assert.deepEqual(packedPackage.cpu, [platform.split('-')[1]]);
    if (platform.endsWith('-gnu')) assert.deepEqual(packedPackage.libc, ['glibc']);
    if (platform.endsWith('-musl')) assert.deepEqual(packedPackage.libc, ['musl']);
    packages.push({ name, target, platform, binary: binaryName, binarySha256: sha256(source),
      tarball: packed.filename, tarballSha256: sha256(tarball), inspection });
  }
  const [packed] = JSON.parse(command('npm', ['pack', '--ignore-scripts', '--json', '--pack-destination', output], root));
  const packedMain = JSON.parse(tarEntry(path.join(output, packed.filename), 'package/package.json'));
  assert.equal(packedMain.name, pkg.name);
  assert.equal(packedMain.version, pkg.version);
  assert.deepEqual(packedMain.optionalDependencies, pkg.optionalDependencies);
  const licenseSha256 = sha256(path.join(root, 'LICENSE'));
  assert.equal(licenseSha256, createHash('sha256').update(tarEntry(
    path.join(output, packed.filename), 'package/LICENSE')).digest('hex'),
    'main tarball license differs from source');
  packages.push({ name: pkg.name, tarball: packed.filename,
    tarballSha256: sha256(path.join(output, packed.filename)) });
  fs.writeFileSync(path.join(output, 'manifest.json'), `${JSON.stringify({
    revision: process.env.GITHUB_SHA || command('git', ['rev-parse', 'HEAD'], root),
    sourceTree: command('git', ['rev-parse', 'HEAD^{tree}'], root),
    licenseSha256, version: pkg.version, packages,
  }, null, 2)}\n`);
  console.log(`Staged ${packages.length} candidate tarballs for ${pkg.version} in ${output}`);
}

if (require.main === module) main();
module.exports = { inspect, tarEntry };
