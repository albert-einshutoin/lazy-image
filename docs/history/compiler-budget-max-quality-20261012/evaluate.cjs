'use strict';

// ponytail: dated #889 lab only; reuse #887 observers and canonical corpus/worker helpers without a benchmark framework.
const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const cp = require('node:child_process'), assert = require('node:assert/strict');
const { performance } = require('node:perf_hooks');
const { sha256, readCorpusManifest, verifyCorpusManifest, isolatedCase } = require('../../../test/helpers/benchmark-corpus');
const { resolveRoot } = require('../../../test/helpers/paths');
const HERE = __dirname;
const DATA = '/tmp/lazy-image-max-quality-889';
const TEMP = '/Volumes/Satechi/Developer/lazy-image-max-quality-889-runs';
const VARIANTS = { current: { root: '/Volumes/Satechi/Developer/lazy-image-max-quality-889', sha: '574b81148b4f2fc30ddab567644e1037c2416343' } };
const RUNTIME_FILES = ['index.js', 'index.mjs', 'index.d.ts', 'lib/artifact-policy.js', 'lib/artifact-compiler.js', 'lib/helpers.js', 'scripts/postbuild-fixup.js'];

function pinned(variant) {
  const v = VARIANTS[variant]; assert(v, 'unknown source');
  const git = args => cp.execFileSync('git', args, { cwd: v.root, encoding: 'utf8' }).trim();
  assert.equal(git(['rev-parse', 'HEAD']), v.sha); assert.equal(git(['status', '--porcelain']), '', 'source must be clean');
  const setup = JSON.parse(fs.readFileSync(path.join(DATA, 'setup.json')));
  assert.equal(setup.initialTrackedStatus, ''); assert(setup.commands.length === 2 && setup.commands.every(c => c.exitCode === 0 && !c.signal && !c.error));
  assert.equal(git(['diff', '--exit-code', setup.sourceSha, v.sha, '--', 'src', 'Cargo.toml', 'Cargo.lock', 'build.rs', 'lib', 'index.js', 'scripts/postbuild-fixup.js']), '', 'native/runtime build inputs changed');
  const native = path.join(v.root, setup.native.path), nativeSha256 = sha256(fs.readFileSync(native));
  assert.equal(nativeSha256, setup.native.sha256);
  for (const file of ['index.js', 'index.d.ts']) assert.equal(sha256(fs.readFileSync(path.join(v.root, file))), setup.generated[file]);
  const api = require(path.join(v.root, 'index.js')); assert(require.cache[native], 'must load checkout-local native');
  const cli = require(path.join(v.root, 'node_modules/@napi-rs/cli/package.json')).version; assert.equal(cli, '3.9.0');
  return { api, identity: { variant, sourceSha: v.sha, buildSourceSha: setup.sourceSha, root: v.root, clean: true,
    nativePath: native, nativeSha256, nativeVersion: api.version(), compilerIdentity: api.compilerIdentity(), napiCli: cli,
    jsSha256: Object.fromEntries(RUNTIME_FILES.map(file => [file, sha256(fs.readFileSync(path.join(v.root, file)))])),
    rustTree: git(['rev-parse', 'HEAD:src']), cargoLockSha256: sha256(fs.readFileSync(path.join(v.root, 'Cargo.lock'))),
    packageLockSha256: sha256(fs.readFileSync(path.join(v.root, 'package-lock.json'))), node: process.version, modules: process.versions.modules,
    platform: process.platform, arch: process.arch, osRelease: os.release(), cpu: os.cpus()[0].model, cpuCount: os.cpus().length,
    memoryBytes: os.totalmem(), runnerSha256: sha256(fs.readFileSync(__filename)),
    observerHelperSha256: sha256(fs.readFileSync(path.join(resolveRoot(), 'test/helpers/benchmark-corpus.js'))) } };
}

function errorRecord(error) {
  const message = error?.message || String(error);
  return { name: error?.name || null, message, code: error?.code || null,
    errorCode: error?.errorCode || message.match(/\[(E\d{3})\]/)?.[1] || null,
    phase: error?.phase || null, category: error?.category ?? error?.errorCategory ?? null,
    cause: error?.cause?.message || null, cleanupError: error?.cleanupError?.message || null };
}

function fixtures() {
  return verifyCorpusManifest(readCorpusManifest(path.join(HERE, 'fixtures.json')), resolveRoot()).entries;
}

async function fileFacts(file) {
  const stat = fs.lstatSync(file);
  assert(stat.isFile() && !stat.isSymbolicLink(), 'output must be regular');
  const hash = require('node:crypto').createHash('sha256');
  for await (const chunk of fs.createReadStream(file)) hash.update(chunk);
  return { bytes: stat.size, sha256: hash.digest('hex') };
}

async function measured(run) {
  const rssStartBytes = process.memoryUsage().rss;
  let peakRssBytes = rssStartBytes, samples = 1;
  const sample = () => { samples++; peakRssBytes = Math.max(peakRssBytes, process.memoryUsage().rss); };
  const timer = setInterval(sample, 10), started = performance.now();
  let value = null, error = null;
  try { value = await run(); } catch (caught) { error = errorRecord(caught); }
  const outerMs = performance.now() - started;
  sample(); clearInterval(timer);
  return { value, error, outerMs, rss: { scope: 'worker process, operation window after module/fixture load; excludes post-verification',
    intervalMs: 10, samples, rssStartBytes, peakRssBytes, rssEndBytes: process.memoryUsage().rss } };
}

async function cli(root, input, output, policy, parent) {
  const policyPath = path.join(parent, 'policy.json');
  fs.writeFileSync(policyPath, JSON.stringify(policy));
  const args = [path.join(root, 'bin/lazy-image.js'), 'compile', input, '--out-dir', output, '--policy', policyPath];
  const started = performance.now(), child = cp.spawn(process.execPath, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'], timeout: 120000 });
  let stdout = '', stderr = '', peakRssBytes = null, samples = 0, missedSamples = 0;
  // Darwin ps reports RSS in KiB. Query only this owned CLI PID, never system-wide.
  const sample = () => {
    try {
      const text = cp.execFileSync('/bin/ps', ['-o', 'rss=', '-p', String(child.pid)], { encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
      if (!/^\d+$/.test(text)) throw Error('RSS unavailable');
      const bytes = Number(text) * 1024;
      peakRssBytes = Math.max(peakRssBytes ?? 0, bytes); samples++;
    } catch { missedSamples++; } // A short-lived/exited child yields no RSS observation.
  };
  sample();
  const timer = setInterval(sample, 100);
  child.stdout.on('data', chunk => { stdout += chunk; });
  child.stderr.on('data', chunk => { stderr += chunk; });
  let exit;
  try { exit = await new Promise((resolve, reject) => { child.once('error', reject); child.once('close', (exitCode, signal) => resolve({ exitCode, signal })); }); }
  finally { clearInterval(timer); }
  const outerMs = performance.now() - started;
  const error = exit.exitCode === 0 ? null : { name: null, message: stderr,
    errorCode: stderr.match(/\[(E\d{3})\]/)?.[1] || null,
    phase: stderr.match(/during (preflight|planning|processing|verification|commit)/)?.[1] || null,
    cause: null, cleanupError: null, detailBoundary: 'CLI diagnostics do not expose cause/cleanupError' };
  return { value: exit.exitCode === 0 ? JSON.parse(stdout) : null, error, outerMs,
    rss: { scope: 'owned CLI child only, includes Node/module startup, maximum sampled RSS; not exact peak', intervalMs: 100, samples, missedSamples, peakRssBytes },
    cliProcess: { command: [process.execPath, ...args], ...exit, stdout, stderr, measurementGuardMs: 120000 } };
}

function tapMetrics(api) {
  const trace = [];
  for (const method of ['toFileWithMetrics', 'toFileTargetBytes']) {
    const original = api.ImageEngine.prototype[method];
    api.ImageEngine.prototype[method] = async function (...args) {
      const call = { method, format: args[1], suppliedOptions: args[2], outcome: null };
      trace.push(call);
      try {
        const result = await original.apply(this, args);
        call.outcome = 'returned'; call.quality = result.quality ?? null;
        call.budgetMet = result.budgetMet ?? null; call.bytesWritten = result.bytesWritten;
        call.metrics = result.metrics ?? null;
        return result;
      } catch (error) { call.outcome = 'rejected'; call.error = errorRecord(error); throw error; }
    };
  }
  return trace;
}

async function publicFile(api, input, output, c) {
  const engine = (await api.ImageEngine.fromPathAsync(input)).sanitize({ policy: 'public-upload' })
    .resize({ width: c.width, fit: 'inside' });
  return engine.toFileWithMetrics(output, c.format, c.quality);
}

async function execute(variant, condition, trial, parent) {
  assert(['compiler', 'cli', 'calibration'].includes(condition.interface));
  const { api, identity } = pinned(variant);
  const entry = fixtures().find(e => e.id === condition.fixture);
  assert(entry, 'fixture absent');
  const input = entry.absolutePath, output = path.join(parent, condition.interface === 'calibration' ? 'output.jpg' : 'output');
  const baselineNames = fs.readdirSync(parent);
  const nativeTrace = condition.interface === 'compiler' ? tapMetrics(api) : [];
  const beforeFile = fs.existsSync(output) ? await fileFacts(output) : null;
  let m;
  if (condition.interface === 'cli') m = await cli(identity.root, input, output, condition.policy, parent);
  else m = await measured(() => condition.interface === 'compiler'
    ? api.compileImage({ inputPath: input, outputDir: output, policy: condition.policy })
    : publicFile(api, input, output, condition));
  const residualNames = fs.readdirSync(parent).filter(n => !baselineNames.includes(n) && n !== 'policy.json');
  const artifacts = [];
  if (!m.error) {
    if (condition.interface === 'compiler' || condition.interface === 'cli') {
      assert.deepEqual(JSON.parse(fs.readFileSync(path.join(output, 'manifest.json'))), m.value);
      for (const a of [...m.value.artifacts, ...(m.value.placeholder ? [m.value.placeholder] : [])]) {
        assert(!a.path.includes('..') && !path.isAbsolute(a.path));
        const actual = await fileFacts(path.join(output, a.path));
        assert.equal(actual.sha256, a.sha256); assert.equal(actual.bytes, a.bytes);
        artifacts.push({ ...a, observed: actual });
      }
    } else artifacts.push({ format: condition.format, quality: m.value.quality ?? condition.quality,
      budgetMet: m.value.budgetMet ?? null, ...await fileFacts(output) });
  }
  const checks = { noUnpublishedStaging: !residualNames.some(n => n !== path.basename(output)),
    rejectedWithoutNewOutput: m.error ? (beforeFile ? (await fileFacts(output)).sha256 === beforeFile.sha256 : !fs.existsSync(output)) : null };
  const postChecks = [];
  if (trial === 0 && condition.id === 'large-compiler-cap' && !m.error) {
    const originalManifest = sha256(fs.readFileSync(path.join(output, 'manifest.json')));
    const guard = await measured(() => api.compileImage({ inputPath: input, outputDir: output, policy: condition.policy }));
    const unchangedArtifacts = (await Promise.all(artifacts.map(async a => (await fileFacts(path.join(output, a.path))).sha256 === a.sha256))).every(Boolean);
    postChecks.push({ kind: 'existing-compiler-output', ...guard, unchangedManifest: sha256(fs.readFileSync(path.join(output, 'manifest.json'))) === originalManifest, unchangedArtifacts,
      remainingSiblingStaging: fs.readdirSync(parent).filter(n => n.includes('lazy-image-staging')) });
  }
  return { schemaVersion: 1, identity, condition, trial, fixture: { id: entry.id, path: entry.path, bytes: entry.bytes, sha256: entry.sha256, license: entry.license, source: entry.source },
    outcome: m.cliProcess?.signal ? 'measurement-failure' : m.error ? 'rejected' : 'returned', error: m.error, outerMs: m.outerMs, rss: m.rss,
    cliProcess: m.cliProcess ?? null, nativeMetrics: condition.interface === 'cli' ? null : (m.value?.metrics ?? null),
    nativeTrace, nativeTraceBoundary: 'delegating public-method observer, no encode/probe count instrumentation',
    encodeCount: null, nativeCumulativeTrusted: variant === 'current', artifacts,
    manifest: condition.interface === 'compiler' || condition.interface === 'cli' ? m.value : null,
    checks, residualNames, postChecks };
}

function caseDirectory(label) {
  fs.mkdirSync(TEMP, { recursive: true });
  return fs.mkdtempSync(path.join(TEMP, label + '-'));
}

function capture(variant, condition, trial, matrixSha256 = null) {
  const parent = caseDirectory(variant + '-' + condition.id + '-' + trial);
  const command = [process.execPath, __filename, '--worker', variant, JSON.stringify(condition), String(trial), parent];
  const startedAt = new Date().toISOString();
  let record;
  try { record = isolatedCase(__filename, [variant, JSON.stringify(condition), trial, parent]); record.workerExitCode = 0; }
  catch (error) { record = { outcome: 'measurement-failure', error: errorRecord(error), workerExitCode: error.status ?? null, signal: error.signal ?? null,
    stdout: error.stdout?.toString() ?? null, stderr: error.stderr?.toString() ?? null, residualNames: fs.readdirSync(parent), condition, trial, variant }; }
  record.execution = { command, startedAt, completedAt: new Date().toISOString(), matrixSha256 };
  return record;
}

function fixedConditions() {
  const primary = [];
  for (const entryPoint of ['compiler', 'cli']) for (const workload of [
    { id: 'large', fixture: 'large', width: 1600, format: 'jpeg', budget: 93782, ceiling: 90 },
    { id: 'avif', fixture: 'coffee', width: 320, format: 'avif', budget: 12850, ceiling: 60 },
  ]) for (const cap of [false, true]) primary.push({ ...workload, id: workload.id + '-' + entryPoint + (cap ? '-cap' : '-default'), interface: entryPoint, cap,
    policy: { widths: [workload.width], formats: [workload.format], placeholder: false, budgets: [{ width: workload.width, format: workload.format,
      maxBytes: workload.budget, ...(cap ? { maxQuality: workload.ceiling } : {}) }] } });
  const regression = [
    { id: 'webp-cap', fixture: 'ui-1', interface: 'compiler', policy: { widths: [320], formats: ['webp'], placeholder: false, budgets: [{ width: 320, format: 'webp', maxBytes: 2218, maxQuality: 60 }] } },
    { id: 'jpeg-none', fixture: 'coffee', interface: 'compiler', policy: { widths: [320], formats: ['jpeg'], placeholder: false } },
    ...[100, 30].map(maxQuality => ({ id: 'nonmonotonic-' + maxQuality, fixture: 'control', interface: 'compiler', policy: { widths: [64], formats: ['jpeg'], placeholder: false,
      budgets: [{ width: 64, format: 'jpeg', maxBytes: 346, maxQuality }] } })),
  ];
  const proof = [...Array.from({ length: 31 }, (_, i) => ({ id: 'large-q' + (90-i), fixture: 'large', format: 'jpeg', width: 1600, quality: 90-i })),
    { id: 'avif-q60', fixture: 'coffee', format: 'avif', width: 320, quality: 60 },
    { id: 'webp-q60', fixture: 'ui-1', format: 'webp', width: 320, quality: 60 },
    ...[30, 100].map(quality => ({ id: 'control-q' + quality, fixture: 'control', format: 'jpeg', width: 64, quality })),
  ].map(c => ({ ...c, interface: 'calibration' }));
  return { primary, regression, proof };
}

async function main() {
  if (process.argv[2] === '--worker') {
    const [variant, c, trial, parent] = process.argv.slice(3);
    process.stdout.write(JSON.stringify(await execute(variant, JSON.parse(c), Number(trial), parent))); return;
  }
  const mode = process.argv[2]; assert(['--freeze', '--run', '--proof'].includes(mode));
  fs.mkdirSync(DATA, { recursive: true });
  const matrixPath = path.join(DATA, 'matrix.json');
  if (mode === '--freeze') {
    const { identity } = pinned('current'); fixtures();
    const matrix = { schemaVersion: 1, issue: 889, trials: 3, concurrency: 1, variants: VARIANTS, identity,
      runnerSha256: sha256(fs.readFileSync(__filename)), fixturesSha256: sha256(fs.readFileSync(path.join(HERE, 'fixtures.json'))), ...fixedConditions() };
    fs.writeFileSync(matrixPath, JSON.stringify(matrix, null, 2) + '\n', { flag: 'wx' }); console.log('Frozen 8 primary, 4 regression, 35 independent proof conditions.'); return;
  }
  const matrix = JSON.parse(fs.readFileSync(matrixPath)), matrixSha = sha256(fs.readFileSync(matrixPath));
  assert.equal(matrix.runnerSha256, sha256(fs.readFileSync(__filename))); assert.equal(matrix.fixturesSha256, sha256(fs.readFileSync(path.join(HERE, 'fixtures.json'))));
  const outputGroup = (group, c, trial) => {
    const folder = path.join(DATA, group); fs.mkdirSync(folder, { recursive: true });
    const target = path.join(folder, c.id + '-' + trial + '.json'); assert(!fs.existsSync(target), 'existing record protected');
    const record = capture('current', c, trial, matrixSha); record.group = group;
    fs.writeFileSync(target, JSON.stringify(record, null, 2) + '\n', { flag: 'wx' });
    console.log(group + '/' + c.id + '/' + trial + ': ' + record.outcome + ' ' + (record.error?.errorCode || '') + ' ' + (record.outerMs?.toFixed(1) ?? 'unknown') + 'ms');
  };
  if (mode === '--proof') { for (const c of matrix.proof) outputGroup('proof', c, 0); }
  else {
    for (let t = 0; t < 3; t++) for (let i = 0; i < matrix.primary.length; i += 2)
      for (const c of t % 2 ? matrix.primary.slice(i, i+2).reverse() : matrix.primary.slice(i, i+2)) outputGroup('raw', c, t);
    for (const c of matrix.regression) outputGroup('regression', c, 0);
  }
}
main().catch(error => { console.error(error); process.exitCode = 1; });
