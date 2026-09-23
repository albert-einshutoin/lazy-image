// Run from a checkout, but install and load only public npm packages in a fresh temp directory.
const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const { createHash } = require('node:crypto');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { createRequire } = require('node:module');

const REGISTRY = 'https://registry.npmjs.org/';
const NAME = '@alberteinshutoin/lazy-image';
const VERSION = '1.3.0';
const FIXTURE = 'test/fixtures/test_100KB_1057x1057.jpg';
const FIXTURE_SHA256 = '6dd5004e9cecaaff5a9b315410b0762c4b27b495379f198bfbbfa496464b6f90';
const POLICY = { widths: [320, 640], formats: ['webp'], placeholder: false };
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
  checkoutRevision: process.env.GITHUB_SHA || null,
  scriptRevision: process.env.SMOKE_SCRIPT_REVISION || null,
  package: `${NAME}@${VERSION}`,
  registry: REGISTRY,
  expectedPlatform: expected,
  runtime: { platform: process.platform, arch: process.arch, osRelease: os.release(), osVersion: os.version(), node: process.version, libc: null },
  host: { runner: process.env.SMOKE_HOST_RUNNER || null, os: process.env.SMOKE_HOST_OS || null, arch: process.env.SMOKE_HOST_ARCH || null },
  fixture: { path: FIXTURE, sha256: FIXTURE_SHA256 },
  policy: POLICY,
  commands: [],
};

function sha256(bytes) { return createHash('sha256').update(bytes).digest('hex'); }

function run(command, args, cwd) {
  report.commands.push([command, ...args].join(' '));
  const result = spawnSync(command, args, {
    cwd, encoding: 'utf8', timeout: 180_000, maxBuffer: 4 * 1024 * 1024,
    shell: process.platform === 'win32' && command.endsWith('.cmd'),
    env: { ...process.env, npm_config_registry: REGISTRY },
  });
  if (result.stdout) process.stdout.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  if (result.error || result.status !== 0) {
    throw new Error(`${command} exited ${result.status}: ${result.error?.message || result.stderr?.slice(-1000)}`);
  }
  return result.stdout.trim();
}

function assertArtifactSet(manifest, outputDir, inspect) {
  assert.equal(manifest.compiler.version, VERSION);
  assert.equal(manifest.source.sha256, FIXTURE_SHA256);
  assert.equal(manifest.artifacts.length, 2);
  assert.deepEqual(manifest.artifacts.map(a => [a.format, a.width, a.height]), [
    ['webp', 320, 320], ['webp', 640, 640],
  ]);
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

async function main() {
  assert.ok(PLATFORMS[expected], `unsupported expected platform: ${expected}`);
  assert.equal(process.env.NAPI_RS_NATIVE_LIBRARY_PATH, undefined);
  assert.equal(process.env.NODE_PATH, undefined);
  const [platform, arch, packageSuffix] = PLATFORMS[expected];
  assert.equal(process.platform, platform);
  assert.equal(process.arch, arch);
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
  run(npm, ['install', '--save-exact', `${NAME}@${VERSION}`, `--registry=${REGISTRY}`, '--no-audit', '--no-fund'], temp);

  const root = path.join(temp, 'node_modules', '@alberteinshutoin');
  const mainDir = path.join(root, 'lazy-image');
  const platformDir = path.join(root, `lazy-image-${packageSuffix}`);
  const lock = JSON.parse(fs.readFileSync(path.join(temp, 'package-lock.json'), 'utf8'));
  report.resolved = {};
  for (const [name, dir] of [[NAME, mainDir], [`${NAME}-${packageSuffix}`, platformDir]]) {
    const pkg = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8'));
    const entry = lock.packages[`node_modules/${name}`];
    const metadata = JSON.parse(run(npm, ['view', `${name}@${VERSION}`, 'dist', '--json', `--registry=${REGISTRY}`], temp));
    assert.equal(pkg.name, name);
    assert.equal(pkg.version, VERSION);
    assert.equal(entry.version, VERSION);
    assert.equal(entry.integrity, metadata.integrity);
    assert.equal(entry.resolved, metadata.tarball);
    assert.ok(metadata.tarball.startsWith(REGISTRY));
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
  report.status = 'PASS';
}

main().catch(error => {
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
