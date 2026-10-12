const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');

const pkg = require('../package.json');

function selectPackages(packages, kind) {
  assert.ok(['platform', 'main', 'wasm'].includes(kind), 'expected platform, main, or wasm');
  const names = kind === 'platform' ? Object.keys(pkg.optionalDependencies).sort()
    : [kind === 'wasm' ? `${pkg.name}-wasm` : pkg.name];
  const selected = packages.filter(entry => names.includes(entry.name));
  assert.deepEqual(selected.map(entry => entry.name).sort(), names, 'missing or duplicate candidate package');
  return selected;
}

function npm(args) {
  const result = spawnSync('npm', args, { encoding: 'utf8', stdio: ['inherit', 'pipe', 'pipe'] });
  if (result.error) throw result.error;
  return result;
}

function main() {
  const directory = path.resolve(process.argv[2] || 'candidate');
  const kind = process.argv[3];
  const manifest = JSON.parse(fs.readFileSync(path.join(directory, 'manifest.json'), 'utf8'));
  assert.equal(manifest.revision, process.env.GITHUB_SHA, 'candidate revision differs from release tag');
  assert.equal(manifest.version, pkg.version);
  for (const item of selectPackages(manifest.packages, kind)) {
    const file = path.join(directory, item.tarball);
    const bytes = fs.readFileSync(file);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), item.tarballSha256,
      `${item.name}: candidate tarball changed`);
    const current = npm(['view', `${item.name}@${manifest.version}`, 'dist.integrity', '--json',
      '--registry=https://registry.npmjs.org/']);
    const integrity = `sha512-${createHash('sha512').update(bytes).digest('base64')}`;
    if (current.status === 0) {
      assert.equal(JSON.parse(current.stdout), integrity, `${item.name}: published version has different bytes`);
      console.log(`${item.name}@${manifest.version} already published with identical integrity`);
      continue;
    }
    assert.match(current.stderr, /E404/, `${item.name}: registry lookup failed for a reason other than absent version`);
    const result = npm(['publish', file, '--access', 'public', '--provenance', '--ignore-scripts',
      '--registry=https://registry.npmjs.org/']);
    if (result.stdout) process.stdout.write(result.stdout);
    if (result.stderr) process.stderr.write(result.stderr);
    if (result.status !== 0) process.exit(result.status || 1);
    console.log(`${item.name}@${manifest.version} published from ${item.tarball} (${item.tarballSha256})`);
  }
}

if (require.main === module) main();
module.exports = { selectPackages };
