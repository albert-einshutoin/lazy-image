// Run from a checkout; install and load registry packages or candidate tarballs in a fresh temp directory.
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createRequire } = require('node:module');

const REGISTRY = 'https://registry.npmjs.org/';
const NAME = '@alberteinshutoin/lazy-image';
const VERSION = process.env.SMOKE_VERSION;
const candidateDir = process.env.SMOKE_CANDIDATE_DIR;
const FIXTURE = 'test/fixtures/test_100KB_1057x1057.jpg';
const FIXTURE_SHA256 = '6dd5004e9cecaaff5a9b315410b0762c4b27b495379f198bfbbfa496464b6f90';
const POLICY = { widths: [320, 640], formats: ['webp'], placeholder: false };
const MAX_QUALITY_POLICY = { widths: [64], formats: ['webp'], placeholder: false,
  budgets: [{ width: 64, format: 'webp', maxBytes: 1024 * 1024, maxQuality: 30 }] };
const PLATFORMS = {
  'darwin-arm64': ['darwin', 'arm64', 'darwin-arm64'],
  'darwin-x64': ['darwin', 'x64', 'darwin-x64'],
  'linux-x64-gnu': ['linux', 'x64', 'linux-x64-gnu'],
  'linux-arm64-gnu': ['linux', 'arm64', 'linux-arm64-gnu'],
  'linux-x64-musl': ['linux', 'x64', 'linux-x64-musl'],
  'win32-x64-msvc': ['win32', 'x64', 'win32-x64-msvc'],
};
const expected = process.env.SMOKE_EXPECTED_PLATFORM;
const reportPath = process.env.SMOKE_REPORT_PATH;
const report = {
  status: 'FAIL',
  startedAt: new Date().toISOString(),
  checkoutRevision: process.env.SMOKE_SCRIPT_REVISION || null,
  eventRevision: process.env.GITHUB_SHA || null,
  scriptRevision: process.env.SMOKE_SCRIPT_REVISION || null,
  package: `${NAME}@${VERSION}`,
  registry: REGISTRY,
  expectedPlatform: expected,
  source: candidateDir ? 'candidate-tarball' : 'published-registry',
  runtime: { platform: process.platform, arch: process.arch, osRelease: os.release(), osVersion: os.version(), node: process.version, libc: null },
  host: { runner: process.env.SMOKE_HOST_RUNNER || null, os: process.env.SMOKE_HOST_OS || null, arch: process.env.SMOKE_HOST_ARCH || null },
  fixture: { path: FIXTURE, sha256: FIXTURE_SHA256 },
  policy: POLICY,
  commands: [],
  commandResults: [],
};

function sha256(bytes) { return createHash('sha256').update(bytes).digest('hex'); }

function run(command, args, cwd, expectedExitCode = 0) {
  report.commands.push([command, ...args].join(' '));
  const result = spawnSync(command, args, {
    cwd, encoding: 'utf8', timeout: 180_000, maxBuffer: 4 * 1024 * 1024,
    shell: process.platform === 'win32' && command.endsWith('.cmd'),
    env: { ...process.env, npm_config_registry: REGISTRY },
  });
  report.commandResults.push({ command: [command, ...args], exitCode: result.status,
    signal: result.signal, expectedExitCode, error: result.error?.message || null });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error || result.signal || result.status !== expectedExitCode) {
    throw new Error(`${command} exited ${result.status}: ${result.error?.message || result.stderr?.slice(-1000)}`);
  }
  if (expectedExitCode !== 0) {
    assert.equal(result.stdout.trim(), '', 'invalid policy must not return a manifest');
    assert.match(result.stderr, /\[E400\]/, 'invalid quality ceiling must be a policy error');
  }
  return result.stdout.trim();
}

function assertArtifactSet(manifest, outputDir, inspect, dimensions = [['webp', 320, 320], ['webp', 640, 640]]) {
  assert.equal(manifest.compiler.version, VERSION);
  assert.equal(manifest.source.sha256, FIXTURE_SHA256);
  assert.equal(manifest.artifacts.length, dimensions.length);
  assert.deepEqual(manifest.artifacts.map(a => [a.format, a.width, a.height]), dimensions);
  assert.deepEqual(JSON.parse(fs.readFileSync(path.join(outputDir, 'manifest.json'), 'utf8')), manifest);
  for (const artifact of manifest.artifacts) {
    const file = path.resolve(outputDir, artifact.path);
    assert.ok(file.startsWith(outputDir + path.sep), 'artifact escaped output directory');
    const bytes = fs.readFileSync(file);
    const metadata = inspect(bytes);
    assert.equal(bytes.length, artifact.bytes);
    assert.equal(sha256(bytes), artifact.sha256);
    assert.equal(metadata.format, artifact.format);
    assert.equal(metadata.width, artifact.width);
    assert.equal(metadata.height, artifact.height);
  }
  return manifest.artifacts.map(({ path: name, format, width, height, bytes, sha256: hash }) =>
    ({ path: name, format, width, height, bytes, sha256: hash }));
}

function assertBudgetedQuality(artifact, budget) {
  // With the smoke's floor=ceiling=30 and ample bytes, exactly q30 must win.
  assert(Number.isInteger(artifact.quality));
  assert.equal(artifact.quality, budget.maxQuality, 'quality ceiling was ignored');
  assert.equal(artifact.targetBytes, budget.maxBytes);
  assert(Number.isInteger(artifact.bytes) && artifact.bytes > 0 && artifact.bytes <= budget.maxBytes);
}

async function main() {
  assert.match(VERSION || '', /^\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.+-]+)?$/,
    'SMOKE_VERSION must specify the tested version');
  assert.ok(PLATFORMS[expected], `unsupported expected platform: ${expected}`);
  assert.equal(process.env.NAPI_RS_NATIVE_LIBRARY_PATH, undefined);
  assert.equal(process.env.NODE_PATH, undefined);
  const [platform, arch, packageSuffix] = PLATFORMS[expected];
  assert.equal(process.platform, platform);
  assert.equal(process.arch, arch);
  const [major, minor, patch] = process.versions.node.split('.').map(Number);
  const minimum = major === 22 ? [23, 4] : major === 24 ? [22, 1] : null;
  assert.ok(minimum && (minor > minimum[0] || (minor === minimum[0] && patch >= minimum[1])),
    'Native release smoke requires patched Node 22.23.4+ or 24.22.1+');
  const glibc = process.report?.getReport()?.header?.glibcVersionRuntime;
  report.runtime.libc = platform === 'linux' ? (glibc ? `glibc ${glibc}` :
    fs.existsSync('/etc/alpine-release') ? `musl (Alpine ${fs.readFileSync('/etc/alpine-release', 'utf8').trim()})` : 'unknown') : 'not applicable';
  if (expected.endsWith('-gnu')) assert.ok(glibc, 'glibc runtime not found');
  if (expected.endsWith('-musl')) {
    assert.equal(glibc, undefined, 'glibc runtime found in musl case');
    assert.ok(fs.existsSync('/etc/alpine-release'), 'Alpine musl runtime not found');
  }
  const npm = process.platform === 'win32' ? 'npm.cmd' : 'npm';
  report.runtime.npm = run(npm, ['--version']);
  const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-registry-smoke-'));
  const checkout = path.resolve(__dirname, '..');
  assert.ok(!temp.startsWith(checkout + path.sep));
  report.installDirectory = temp;
  fs.writeFileSync(path.join(temp, 'package.json'), '{"private":true}\n');
  const inputPath = path.join(temp, 'input.jpg');
  fs.copyFileSync(path.join(checkout, FIXTURE), inputPath);
  assert.equal(sha256(fs.readFileSync(inputPath)), FIXTURE_SHA256);
  let candidate;
  if (candidateDir) {
    candidate = JSON.parse(fs.readFileSync(path.join(candidateDir, 'manifest.json'), 'utf8'));
    assert.equal(candidate.version, VERSION);
    if (process.env.GITHUB_SHA) assert.equal(candidate.revision, process.env.GITHUB_SHA);
    const selected = [NAME, `${NAME}-${packageSuffix}`].map(name => {
      const item = candidate.packages.find(entry => entry.name === name);
      assert.ok(item, `candidate package missing: ${name}`);
      const tarball = path.join(candidateDir, item.tarball);
      assert.equal(sha256(fs.readFileSync(tarball)), item.tarballSha256);
      return tarball;
    });
    run(npm, ['install', '--save-exact', '--omit=optional', ...selected, `--registry=${REGISTRY}`, '--no-audit', '--no-fund'], temp);
    report.candidate = { revision: candidate.revision, tarballs: selected };
  } else {
    run(npm, ['install', '--save-exact', `${NAME}@${VERSION}`, `--registry=${REGISTRY}`, '--no-audit', '--no-fund'], temp);
  }

  const root = path.join(temp, 'node_modules', '@alberteinshutoin');
  const mainDir = path.join(root, 'lazy-image');
  const platformDir = path.join(root, `lazy-image-${packageSuffix}`);
  report.licenseSha256 = sha256(fs.readFileSync(path.join(mainDir, 'LICENSE')));
  if (candidate) assert.equal(report.licenseSha256, candidate.licenseSha256);
  const lock = JSON.parse(fs.readFileSync(path.join(temp, 'package-lock.json'), 'utf8'));
  report.resolved = {};
  for (const [name, dir] of [[NAME, mainDir], [`${NAME}-${packageSuffix}`, platformDir]]) {
    const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    const entry = lock.packages[`node_modules/${name}`];
    assert.equal(pkg.name, name);
    assert.equal(pkg.version, VERSION);
    assert.equal(entry.version, VERSION);
    if (candidate) {
      const item = candidate.packages.find(record => record.name === name);
      const tarball = path.join(candidateDir, item.tarball);
      assert.equal(entry.integrity, `sha512-${createHash('sha512').update(fs.readFileSync(tarball)).digest('base64')}`);
      if (item.binary) assert.equal(sha256(fs.readFileSync(path.join(dir, item.binary))), item.binarySha256);
    } else {
      const metadata = JSON.parse(run(npm, ['view', `${name}@${VERSION}`, 'dist', '--json', `--registry=${REGISTRY}`], temp));
      assert.equal(entry.integrity, metadata.integrity);
      assert.equal(entry.resolved, metadata.tarball);
      assert.ok(metadata.tarball.startsWith(REGISTRY));
    }
    report.resolved[name] = { version: pkg.version, tarball: entry.resolved, integrity: entry.integrity };
  }

  const installedRequire = createRequire(path.join(temp, 'package.json'));
  const mainEntry = installedRequire.resolve(NAME);
  assert.ok(fs.realpathSync(mainEntry).startsWith(fs.realpathSync(mainDir) + path.sep));
  const lib = installedRequire(NAME);
  const binding = Object.keys(require.cache).find(file => file.endsWith('.node'));
  assert.ok(binding, 'native binding not loaded');
  assert.ok(fs.realpathSync(binding).startsWith(fs.realpathSync(platformDir) + path.sep));
  report.loaded = { entry: mainEntry, binding };

  assert.equal('preset' in lib.ImageEngine.prototype, false, 'v2 removed the mutating preset() method');
  const engine = lib.ImageEngine.fromPath(inputPath);
  const encoded = await engine.encode({ preset: 'thumbnail', metrics: true });
  assert.ok(Buffer.isBuffer(encoded.data) && encoded.data.length > 0);
  assert.equal(encoded.metrics.version, '1.1.0');
  const thumbnail = lib.inspect(encoded.data);
  assert.deepEqual([thumbnail.format, thumbnail.width, thumbnail.height], ['webp', 150, 150]);
  const avatar = lib.inspect(await engine.toBufferWithPreset('avatar'));
  assert.deepEqual([avatar.format, avatar.width, avatar.height], ['webp', 200, 200]);
  const metricsPreset = await engine.toBufferWithMetricsPreset('thumbnail');
  assert.equal(metricsPreset.metrics.version, '1.1.0');
  const encodedPath = path.join(temp, 'encode-preset.webp');
  const file = await engine.encodeToFile(encodedPath, { preset: 'avatar', metrics: true });
  assert.equal(file.bytesWritten, fs.statSync(encodedPath).size);
  const helperPath = path.join(temp, 'helper-preset.webp');
  assert.equal(await engine.toFileWithPreset(helperPath, 'thumbnail'), fs.statSync(helperPath).size);
  const reused = lib.inspect(await engine.toBuffer('webp', 80));
  assert.deepEqual([reused.width, reused.height], [1057, 1057], 'preset outputs must not leak resize state');
  report.v2Migration = { removedPreset: true, encode: true, encodeToFile: true,
    toBufferWithPreset: true, toBufferWithMetricsPreset: true, toFileWithPreset: true,
    reusedDimensions: [reused.width, reused.height], metricsVersion: encoded.metrics.version };

  const cli = path.join(mainDir, 'bin', 'lazy-image.js');
  const bin = path.join(temp, 'node_modules', '.bin', process.platform === 'win32' ? 'lazy-image.cmd' : 'lazy-image');
  assert.ok(fs.existsSync(bin), 'installed bin shim missing');
  assert.equal(JSON.parse(fs.readFileSync(path.join(mainDir, 'package.json'))).bin['lazy-image'], './bin/lazy-image.js');
  assert.ok(fs.realpathSync(cli).startsWith(fs.realpathSync(mainDir) + path.sep));
  report.loaded.bin = bin;
  report.loaded.binTarget = cli;
  const policyPath = path.join(temp, 'policy.json');
  fs.writeFileSync(policyPath, JSON.stringify(POLICY));
  const apiDir = path.join(temp, 'api-output');
  const cliDir = path.join(temp, 'cli-output');
  const apiManifest = await lib.compileImage({ inputPath, outputDir: apiDir, policy: POLICY });
  report.apiArtifacts = assertArtifactSet(apiManifest, apiDir, lib.inspect);
  const cliManifest = JSON.parse(run(bin, ['compile', inputPath, '--out-dir', cliDir, '--policy', policyPath], temp));
  report.cliArtifacts = assertArtifactSet(cliManifest, cliDir, lib.inspect);
  assert.deepEqual(cliManifest, apiManifest);
  report.results = { binding: 'PASS', api: 'PASS', cli: 'PASS', artifacts: 'PASS', apiCliParity: 'PASS' };

  const cappedApiDir = path.join(temp, 'max-quality-api');
  const cappedCliDir = path.join(temp, 'max-quality-cli');
  const cappedApi = await lib.compileImage({ inputPath, outputDir: cappedApiDir, policy: MAX_QUALITY_POLICY });
  fs.writeFileSync(policyPath, JSON.stringify(MAX_QUALITY_POLICY));
  const cappedCli = JSON.parse(run(bin, ['compile', inputPath, '--out-dir', cappedCliDir, '--policy', policyPath], temp));
  const dimensions = [['webp', 64, 64]];
  const cappedArtifacts = (manifest, dir) => {
    const artifacts = assertArtifactSet(manifest, dir, lib.inspect, dimensions);
    assertBudgetedQuality(manifest.artifacts[0], MAX_QUALITY_POLICY.budgets[0]);
    return artifacts.map((artifact, index) => ({ ...artifact, quality: manifest.artifacts[index].quality,
      targetBytes: manifest.artifacts[index].targetBytes }));
  };
  report.maxQuality = { policy: MAX_QUALITY_POLICY, apiArtifacts: cappedArtifacts(cappedApi, cappedApiDir),
    cliArtifacts: cappedArtifacts(cappedCli, cappedCliDir), invalid: [] };
  assert.deepEqual(cappedCli, cappedApi);
  for (const [index, maxQuality] of [29, 101, '30'].entries()) {
    const invalidPolicy = { ...MAX_QUALITY_POLICY,
      budgets: [{ ...MAX_QUALITY_POLICY.budgets[0], maxQuality }] };
    const invalidApiDir = path.join(temp, `invalid-max-quality-api-${index}`);
    const invalidCliDir = path.join(temp, `invalid-max-quality-cli-${index}`);
    await assert.rejects(() => lib.compileImage({ inputPath, outputDir: invalidApiDir, policy: invalidPolicy }), error => {
      assert.equal(error.errorCode, 'E400'); assert.equal(error.phase, 'planning'); return true;
    });
    fs.writeFileSync(policyPath, JSON.stringify(invalidPolicy));
    run(bin, ['compile', inputPath, '--out-dir', invalidCliDir, '--policy', policyPath], temp, 1);
    assert.equal(fs.existsSync(invalidApiDir), false); assert.equal(fs.existsSync(invalidCliDir), false);
    assert.deepEqual(fs.readdirSync(temp).filter(name => name.includes('lazy-image-staging')), []);
    report.maxQuality.invalid.push({ maxQuality, api: 'E400', cli: 'E400', unpublished: true, remainingSiblingStaging: [] });
  }
  report.results.maxQuality = 'PASS';
  report.results.invalidMaxQuality = 'PASS';
  // Use the actual published old platform artifact, and intercept the .node
  // evaluation boundary. No old native initializer is allowed to run.
  const negative = path.join(temp, 'stale-consumer');
  fs.mkdirSync(negative);
  fs.writeFileSync(path.join(negative, 'package.json'), '{"private":true}\n');
  const oldName = `${NAME}-${packageSuffix}`;
  run(npm, ['install', '--save-exact', '--ignore-scripts', '--omit=optional', `${oldName}@1.5.0`,
    `--registry=${REGISTRY}`, '--no-audit', '--no-fund'], negative);
  const oldLock = JSON.parse(fs.readFileSync(path.join(negative, 'package-lock.json'))).packages[`node_modules/${oldName}`];
  const oldDist = JSON.parse(run(npm, ['view', `${oldName}@1.5.0`, 'dist', '--json', `--registry=${REGISTRY}`], negative));
  assert.equal(oldLock.integrity, oldDist.integrity);
  assert.equal(oldLock.resolved, oldDist.tarball);
  const oldDir = path.join(negative, 'node_modules', '@alberteinshutoin', `lazy-image-${packageSuffix}`);
  const oldPackage = JSON.parse(fs.readFileSync(path.join(oldDir, 'package.json')));
  assert.equal(oldPackage.version, '1.5.0');
  fs.cpSync(mainDir, path.join(negative, 'node_modules', '@alberteinshutoin', 'lazy-image'), { recursive: true });
  const probe = `const assert = require('node:assert/strict'); let evaluated = 0;
    delete process.env.NAPI_RS_ENFORCE_VERSION_CHECK;
    require('node:module')._extensions['.node'] = () => { evaluated++; throw Error('old addon evaluated'); };
    assert.throws(() => require(${JSON.stringify(NAME)}), /Native binding package version mismatch/);
    assert.equal(evaluated, 0); console.log('stale native rejected before evaluation');`;
  run(process.execPath, ['-e', probe], negative);
  report.staleNative = { version: oldPackage.version, package: oldName, tarball: oldDist.tarball, integrity: oldDist.integrity,
    binarySha256: sha256(fs.readFileSync(path.join(oldDir, oldPackage.main))), addonEvaluations: 0, defaultEnvironment: true };
  report.results.staleNative = 'PASS';
  report.status = 'PASS';
}

if (require.main === module) main().catch(error => {
  report.reason = error.stack || String(error);
  console.error(error);
  process.exitCode = 1;
}).finally(() => {
  report.finishedAt = new Date().toISOString();
  if (reportPath) {
    fs.mkdirSync(path.dirname(reportPath), { recursive: true });
    fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
  }
  console.log(`registry-native-smoke: ${report.status} ${expected} ${process.version}`);
});

module.exports = { assertBudgetedQuality };
