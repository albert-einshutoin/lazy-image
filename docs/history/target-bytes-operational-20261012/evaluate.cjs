'use strict';

// Scoped #887 laboratory runner. It delegates every encode/publication to public APIs.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const cp = require('node:child_process');
const assert = require('node:assert/strict');
const { performance } = require('node:perf_hooks');
const { sha256, readCorpusManifest, verifyCorpusManifest, isolatedCase } = require('../../../test/helpers/benchmark-corpus');
const { resolveRoot } = require('../../../test/helpers/paths');

const HERE = __dirname;
const DATA = '/tmp/lazy-image-operational-887';
const TEMP = '/Volumes/Satechi/Developer/lazy-image-operational-887-runs';
const VARIANTS = {
  current: { root: '/Volumes/Satechi/Developer/lazy-image-evaluation-20261012-main', sha: '1962f233299aa10d6053230c1af1605faeaddc5d' },
  before: { root: '/Volumes/Satechi/Developer/lazy-image-evaluation-20261012-before', sha: '21a15bc99d4c5258f640054e00e9958994e124a3' },
};
const WORKLOADS = [
  { id: 'photo', fixture: 'coffee', format: 'jpeg', width: 320 },
  { id: 'ui', fixture: 'ui-1', format: 'webp', width: 320 },
  { id: 'large', fixture: 'large', format: 'jpeg', width: 1600 },
  { id: 'avif', fixture: 'coffee', format: 'avif', width: 320 },
];

function errorRecord(error) {
  const message = error?.message || String(error);
  return { name: error?.name || null, message, code: error?.code || null,
    errorCode: error?.errorCode || message.match(/\[(E\d{3})\]/)?.[1] || null,
    phase: error?.phase || null, category: error?.category ?? error?.errorCategory ?? null,
    cause: error?.cause?.message || null, cleanupError: error?.cleanupError?.message || null };
}

function pinned(variant) {
  const v = VARIANTS[variant];
  assert(v, 'unknown variant');
  const git = args => cp.execFileSync('git', args, { cwd: v.root, encoding: 'utf8' }).trim();
  assert.equal(git(['rev-parse', 'HEAD']), v.sha);
  assert.equal(git(['status', '--porcelain']), '', 'measurement source must be clean');
  const native = path.join(v.root, 'lazy-image.darwin-arm64.node');
  const nativeSha256 = sha256(fs.readFileSync(native));
  const api = require(path.join(v.root, 'index.js'));
  assert(require.cache[native], 'the checkout-local native must be loaded');
  const cli = require(path.join(v.root, 'node_modules/@napi-rs/cli/package.json')).version;
  assert.equal(cli, '3.9.0');
  return { api, identity: { variant, sourceSha: v.sha, root: v.root, clean: true,
    nativePath: native, nativeSha256, nativeVersion: api.version(), compilerIdentity: api.compilerIdentity(),
    napiCli: cli, packageVersion: require(path.join(v.root, 'package.json')).version,
    cargoLockSha256: sha256(fs.readFileSync(path.join(v.root, 'Cargo.lock'))),
    packageLockSha256: sha256(fs.readFileSync(path.join(v.root, 'package-lock.json'))),
    node: process.version, modules: process.versions.modules, platform: process.platform,
    arch: process.arch, osRelease: os.release(), cpu: os.cpus()[0].model, cpuCount: os.cpus().length,
    memoryBytes: os.totalmem(), runnerSha256: sha256(fs.readFileSync(__filename)) } };
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
  const started = performance.now(), child = cp.spawn(process.execPath, args, { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
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
    cliProcess: { command: [process.execPath, ...args], ...exit, stdout, stderr } };
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
  if (c.interface === 'calibration') return engine.toFileWithMetrics(output, c.format, c.quality);
  return engine.toFileTargetBytes(output, c.format, { targetBytes: c.budget,
    minQuality: c.minQuality, maxQuality: c.maxQuality, strict: true });
}

async function execute(variant, condition, trial, parent) {
  const { api, identity } = pinned(variant);
  const entry = fixtures().find(e => e.id === condition.fixture);
  assert(entry, 'fixture absent');
  const input = entry.absolutePath, output = path.join(parent, condition.interface === 'api' || condition.interface === 'calibration' ? 'output.jpg' : 'output');
  const baselineNames = fs.readdirSync(parent);
  const nativeTrace = condition.interface === 'compiler' ? tapMetrics(api) : [];
  if (condition.id === 'api-known-max') fs.writeFileSync(output, 'owned existing file sentinel');
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
  if (trial === 0 && condition.id === 'photo-none' && !m.error) {
    const originalManifest = sha256(fs.readFileSync(path.join(output, 'manifest.json')));
    const guard = await measured(() => api.compileImage({ inputPath: input, outputDir: output, policy: condition.policy }));
    const unchangedArtifacts = (await Promise.all(artifacts.map(async a => (await fileFacts(path.join(output, a.path))).sha256 === a.sha256))).every(Boolean);
    postChecks.push({ kind: 'existing-compiler-output', ...guard, unchangedManifest: sha256(fs.readFileSync(path.join(output, 'manifest.json'))) === originalManifest, unchangedArtifacts,
      remainingSiblingStaging: fs.readdirSync(parent).filter(n => n.includes('lazy-image-staging')) });
  }
  if (trial === 0 && condition.id === 'api-known-max') {
    const previous = await fileFacts(output);
    const strict = await measured(() => publicFile(api, input, output, { ...condition, budget: 1 }));
    postChecks.push({ kind: 'strict-existing-file', ...strict, unchangedFile: (await fileFacts(output)).sha256 === previous.sha256 });
    const best = await measured(async () => (await api.ImageEngine.fromPathAsync(input)).sanitize({ policy: 'public-upload' })
      .toBufferTargetBytes('jpeg', { targetBytes: 1, minQuality: 1, maxQuality: 100, qualityFloorPolicy: 'best-effort' }));
    if (best.value) best.value = { quality: best.value.quality, bytes: best.value.data.length, sha256: sha256(best.value.data), budgetMet: best.value.budgetMet, metrics: best.value.metrics };
    postChecks.push({ kind: 'best-effort-unmet-control', ...best });
  }
  return { schemaVersion: 1, identity, condition, trial, fixture: { id: entry.id, path: entry.path, bytes: entry.bytes, sha256: entry.sha256, license: entry.license, source: entry.source },
    outcome: m.error ? 'rejected' : 'returned', error: m.error, outerMs: m.outerMs, rss: m.rss,
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

function conditions(calibration) {
  const list = [];
  for (const w of WORKLOADS.slice(0, 3)) {
    const q60 = calibration.find(r => r.condition.id === w.id + '-q60');
    const q100 = calibration.find(r => r.condition.id === w.id + '-q100');
    assert(q60 && q100 && !q60.error && !q100.error, 'budget calibration must succeed');
    for (const budgetKind of ['none', 'loose', 'tight', 'impossible']) {
      const budget = budgetKind === 'none' ? null : budgetKind === 'impossible' ? 1
        : budgetKind === 'tight' ? q60.artifacts[0].bytes : Math.max(Math.ceil(q100.artifacts[0].bytes * 1.1), q60.artifacts[0].bytes + 1);
      list.push({ ...w, id: w.id + '-' + budgetKind, interface: 'compiler', budgetKind, budget,
        policy: { widths: [w.width], formats: [w.format], placeholder: false,
          ...(budget === null ? {} : { budgets: [{ width: w.width, format: w.format, maxBytes: budget }] }) } });
    }
  }
  list.push({ id: 'cli-build', fixture: 'large', interface: 'cli', budgetKind: 'none', policy: { widths: [320, 1280], formats: ['jpeg', 'webp'], placeholder: true } });
  list.push({ ...list.find(c => c.id === 'ui-tight'), id: 'cli-ui-tight', interface: 'cli' });
  const avif = WORKLOADS[3], q60 = calibration.find(r => r.condition.id === 'avif-q60');
  assert(q60 && !q60.error, 'AVIF byte witness required');
  for (const kind of ['none', 'tight']) list.push({ ...avif, id: 'avif-' + kind, interface: 'compiler', budgetKind: kind,
    budget: kind === 'tight' ? q60.artifacts[0].bytes : null, policy: { widths: [320], formats: ['avif'], placeholder: false,
      ...(kind === 'tight' ? { budgets: [{ width: 320, format: 'avif', maxBytes: q60.artifacts[0].bytes }] } : {}) } });
  list.push({ ...WORKLOADS[2], id: 'api-range-tight', interface: 'api', budgetKind: 'tight', budget: list.find(c => c.id === 'large-tight').budget, minQuality: 40, maxQuality: 90 });
  list.push({ id: 'api-known-max', fixture: 'control', interface: 'api', format: 'jpeg', width: 64, budgetKind: 'known-feasible', budget: 346, minQuality: 1, maxQuality: 100 });
  assert.equal(list.length, 18);
  return list;
}

async function main() {
  fs.mkdirSync(DATA, { recursive: true });
  if (process.argv[2] === '--worker') {
    const [variant, conditionJson, trialText, parent] = process.argv.slice(3);
    process.stdout.write(JSON.stringify(await execute(variant, JSON.parse(conditionJson), Number(trialText), parent)));
    return;
  }
  const phase = process.argv[2];
  assert(['--calibrate', '--run'].includes(phase), 'use --calibrate or --run');
  const runnerSha256 = sha256(fs.readFileSync(__filename));
  if (phase === '--calibrate') {
    assert(!fs.existsSync(path.join(DATA, 'matrix.json')), 'frozen matrix already exists');
    const records = [];
    for (const w of WORKLOADS) for (const quality of [100, 60]) {
      const c = { ...w, id: w.id + '-q' + quality, interface: 'calibration', quality };
      const r = capture('current', c, 0);
      records.push(r); fs.writeFileSync(path.join(DATA, 'calibration.json'), JSON.stringify(records, null, 2) + '\n');
      console.log(c.id + ': ' + (r.error ? r.error.errorCode : r.artifacts[0].bytes + ' B'));
    }
    for (const variant of ['before', 'current']) {
      const c = { id: 'control-q100', fixture: 'control', interface: 'calibration', format: 'jpeg', width: 64, quality: 100 };
      const r = capture(variant, c, 0);
      records.push(r); fs.writeFileSync(path.join(DATA, 'calibration.json'), JSON.stringify(records, null, 2) + '\n');
      assert(r.outcome === 'returned' && r.artifacts[0].bytes === 346, 'known max must remain byte-feasible');
    }
    fs.writeFileSync(path.join(DATA, 'calibration.json'), JSON.stringify(records, null, 2) + '\n');
    const matrix = { schemaVersion: 1, issue: 887, trials: 3, concurrency: 1, variants: VARIANTS,
      runnerSha256, fixturesSha256: sha256(fs.readFileSync(path.join(HERE, 'fixtures.json'))), conditions: conditions(records) };
    fs.writeFileSync(path.join(DATA, 'matrix.json'), JSON.stringify(matrix, null, 2) + '\n');
    console.log('frozen 18-condition matrix SHA256 ' + sha256(fs.readFileSync(path.join(DATA, 'matrix.json'))));
  } else {
    const matrix = JSON.parse(fs.readFileSync(path.join(DATA, 'matrix.json')));
    assert.equal(matrix.runnerSha256, runnerSha256, 'runner changed after freeze');
    assert.equal(matrix.fixturesSha256, sha256(fs.readFileSync(path.join(HERE, 'fixtures.json'))));
    fs.mkdirSync(path.join(DATA, 'raw'), { recursive: true });
    for (const c of matrix.conditions) for (let trial = 0; trial < matrix.trials; trial++) for (const variant of trial % 2 ? ['current', 'before'] : ['before', 'current']) {
      const name = variant + '-' + c.id + '-' + trial + '.json', target = path.join(DATA, 'raw', name);
      assert(!fs.existsSync(target), 'existing trial protected');
      const record = capture(variant, c, trial, sha256(fs.readFileSync(path.join(DATA, 'matrix.json'))));
      fs.writeFileSync(target, JSON.stringify(record, null, 2) + '\n');
      console.log(name + ': ' + record.outcome + ' ' + (record.error?.errorCode || '') + ' ' + (record.outerMs?.toFixed(1) ?? 'unknown') + 'ms');
    }
  }
}

main().catch(error => { console.error(error); process.exitCode = 1; });
