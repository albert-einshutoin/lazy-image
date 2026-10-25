'use strict';

// Reproduce the finite #915 protocol; observed samples are diagnosis only.
const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const cp = require('node:child_process');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { createRequire } = require('node:module');
const { performance, PerformanceObserver, monitorEventLoopDelay } = require('node:perf_hooks');
const harness = require('../../../scripts/bench/paired-comparison.cjs');
const repo = path.resolve(__dirname, '../../..');
const runner = 'test/benchmarks/sharp-comparison.bench.js';
const write = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
const read = file => JSON.parse(fs.readFileSync(file));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = (root, ...args) => cp.execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
const median = values => { const s = [...values].sort((a, b) => a - b); return (s[Math.floor((s.length - 1) / 2)] + s[Math.floor(s.length / 2)]) / 2; };

function assertIdentity(report, expected) {
  assert.equal(report.revision, expected.revision, 'Unexpected measured revision');
  assert.equal(report.sourceTree, expected.sourceTree, 'Unexpected source tree');
  assert.ok(report.modifiedPaths.every(p => p === runner), 'Dirty tracked dependency');
  assert.deepEqual(report.untrackedPaths, [], 'Untracked source');
  assert.equal(report.runnerSha256, expected.runnerSha256, 'Unexpected measurement overlay');
  assert.equal(report.fixture.sha256, expected.fixtureSha256, 'Unexpected fixture');
  assert.equal(report.binding.sha256, expected.bindingSha256, 'Unexpected native binding');
}

function assertContract(suite, contracts, isolated) {
  const complex = suite.outputs.filter(o => o.name === 'Complex Pipeline');
  assert.equal(complex.length, 2);
  if (!contracts.complex) contracts.complex = complex;
  assert.deepEqual(complex, contracts.complex, 'Complex output differs across arms');
  if (!isolated) {
    const full = { outputs: suite.outputs, qualityAxes: suite.qualityAxes };
    if (!contracts.full) contracts.full = full;
    assert.deepEqual(full, contracts.full, 'Ordinary/observed/native output or quality differs');
  }
}

function nativeTiming(metrics, wallMs) {
  for (const key of ['decodeMs', 'opsMs', 'encodeMs', 'totalMs']) assert.ok(Number.isFinite(metrics[key]) && metrics[key] >= 0, 'Invalid native timing');
  assert.ok(metrics.decodeMs + metrics.opsMs + metrics.encodeMs <= metrics.totalMs + 0.1, 'Invalid native stage chronology');
  assert.ok(Number.isFinite(wallMs) && wallMs >= metrics.totalMs, 'Native total exceeds observed wall');
  return { wallMs, wallMinusNativeTotalMs: wallMs - metrics.totalMs,
    scope: 'Native total starts after memory admission, ends before JS resolve; remainder combines earlier API/admission/worker queue/JS scheduling/observation overhead and is not a queue-time measurement.' };
}

function diagnosisSummary(pairs) {
  const rows = [];
  for (const arm of [...new Set(pairs.map(p => p.arm))]) for (const phase of ['first', 'warm']) {
    const selected = pairs.filter(p => p.arm === arm), times = { a: [], b: [] };
    for (const pair of selected) for (const side of ['a', 'b']) times[side].push(median(pair[side].samples.filter(s => phase === 'first' ? s.phase === 'first' : s.phase.startsWith('warm')).map(s => s.timings['Complex Pipeline'].lazy)));
    const ratios = times.b.map((v, i) => v / times.a[i]);
    rows.push({ arm, phase, times, ratios, medianRatio: median(ratios), order: selected.map(p => p.order) });
  }
  const contrasts = [];
  for (const [left, right] of [['original-observed', 'original-passive'], ['isolated-observed', 'isolated-passive'], ['original-passive', 'isolated-passive']]) for (const phase of ['first', 'warm']) for (const side of ['a', 'b']) {
    const a = rows.find(r => r.arm === left && r.phase === phase), b = rows.find(r => r.arm === right && r.phase === phase);
    if (a && b) contrasts.push({ numerator: left, denominator: right, phase, side, ratios: a.times[side].map((v, i) => v / b.times[side][i]), ratioOfMedians: median(a.times[side]) / median(b.times[side]) });
  }
  return { purpose: 'Four pairs per arm localize hypotheses; not performance acceptance or causal proof', rows, contrasts };
}

async function observedSample(root, output, arm) {
  const local = createRequire(path.join(root, 'package.json'));
  const trace = { arm, timeOrigin: performance.timeOrigin, intervals: [], gc: [] };
  const observed = arm.endsWith('observed') || arm === 'native';
  let observer, delay;
  if (observed) {
    observer = new PerformanceObserver(list => {
      for (const e of list.getEntries()) trace.gc.push({ startMs: e.startTime, durationMs: e.duration, detail: e.detail });
    });
    observer.observe({ entryTypes: ['gc'] });
    delay = monitorEventLoopDelay({ resolution: 10 }); delay.enable();
    const snapshot = () => ({ atMs: performance.now(), cpu: process.cpuUsage(), usage: process.resourceUsage(), memory: process.memoryUsage(), elu: performance.eventLoopUtilization() });
    async function record(name, fn) {
      const before = snapshot();
      try { return await fn(); }
      finally { trace.intervals.push({ name, before, after: snapshot() }); }
    }
    const quality = local('./test/helpers/quality.js');
    const metrics = quality.calculateQualityMetrics;
    quality.calculateQualityMetrics = (...args) => record('quality', () => metrics(...args));
    const { ImageEngine } = local('./index');
    const encode = ImageEngine.prototype.toBuffer;
    ImageEngine.prototype.toBuffer = function (...args) {
      return record('native:' + args[0], async () => {
        if (arm !== 'native') return encode.apply(this, args);
        const result = await this.toBufferWithMetrics(...args);
        trace.intervals.push({ name: 'native-metrics:' + args[0], atMs: performance.now(), metrics: result.metrics });
        return result.data;
      });
    };
  }
  try {
    await harness.sample(root, output, { isolated: arm.startsWith('isolated') });
    if (arm === 'native') {
      const calls = trace.intervals.filter(i => i.name.startsWith('native:'));
      const metrics = trace.intervals.filter(i => i.name.startsWith('native-metrics:'));
      assert.equal(calls.length, metrics.length);
      trace.nativeTiming = calls.map((call, i) => ({ ...nativeTiming(metrics[i].metrics, call.after.atMs - call.before.atMs), metrics: metrics[i].metrics }));
    }
  } finally {
    if (observer) {
      await new Promise(resolve => setImmediate(resolve));
      observer.disconnect(); delay.disable();
      trace.eventLoopDelay = { meanMs: delay.mean / 1e6, maxMs: delay.max / 1e6, p99Ms: delay.percentile(99) / 1e6 };
    }
    if (fs.existsSync(output)) write(path.join(output, 'trace.json'), trace);
  }
}

function hostState() {
  const state = JSON.parse(cp.execFileSync('osascript', ['-l', 'JavaScript', '-e', 'ObjC.import("Foundation"); var p=$.NSProcessInfo.processInfo; JSON.stringify({thermalState:Number(p.thermalState),lowPowerMode:!!p.isLowPowerModeEnabled});'], { encoding: 'utf8' }));
  return { ...state, power: cp.execFileSync('pmset', ['-g', 'ps'], { encoding: 'utf8' }), load: os.loadavg(), at: new Date().toISOString() };
}

async function preconditions(output) {
  const probes = [];
  for (let attempt = 0; attempt < 150; attempt++) {
    const state = hostState(), idle = [];
    for (let i = 0; i < 2; i++) {
      const before = os.cpus();
      await new Promise(resolve => setTimeout(resolve, 1000));
      const after = os.cpus(); let all = 0, free = 0;
      after.forEach((cpu, n) => { for (const k of Object.keys(cpu.times)) all += cpu.times[k] - before[n].times[k]; free += cpu.times.idle - before[n].times.idle; });
      idle.push(free / all);
    }
    probes.push({ ...state, idle }); write(path.join(output, 'preconditions.json'), probes);
    if (state.thermalState === 0 && !state.lowPowerMode && state.power.includes('AC Power') && idle.every(v => v >= 0.60)) return;
  }
  throw Error('Finite precondition window exhausted; no image measurement started');
}

function child(root, dir, arm) {
  const fd = fs.openSync(dir + '.log', 'wx');
  const flags = arm.endsWith('observed') || arm === 'native' ? ['--trace-gc-nvp'] : [];
  const result = cp.spawnSync(process.execPath, [...flags, __filename, 'sample', root, dir, arm], { cwd: root,
    env: { ...process.env, UV_THREADPOOL_SIZE: '4' }, stdio: ['ignore', fd, fd] });
  fs.closeSync(fd);
  assert.equal(result.status, 0, `${dir}: sample failed; retain partial report and log`);
  const report = read(path.join(dir, 'sample.json'));
  assert.equal(report.status, 'PASS');
  return report;
}

async function measure(mode, base, candidate, output, referenceContracts) {
  assert.notEqual(fs.realpathSync(base), fs.realpathSync(candidate));
  fs.mkdirSync(output); // Never replace a prior batch.
  const protocol = read(path.join(__dirname, 'protocol.json'));
  const amendment = read(path.join(__dirname, 'protocol-amendment.json'));
  const overlay = fs.readFileSync(path.join(repo, runner));
  const expected = {};
  for (const [side, root] of [['a', base], ['b', candidate]]) {
    assert.equal(git(root, 'rev-parse', 'HEAD'), side === 'a' ? protocol.base : protocol.candidate, 'Unexpected checkout revision');
    const modified = git(root, 'diff', '--name-only', '-z', 'HEAD').split('\0').filter(Boolean);
    assert.ok(modified.every(p => p === runner), 'Dirty tracked dependency');
    assert.equal(git(root, 'ls-files', '--others', '--exclude-standard', '-z'), '', 'Untracked source');
    fs.writeFileSync(path.join(root, runner), overlay);
    expected[side] = { revision: side === 'a' ? protocol.base : protocol.candidate, sourceTree: git(root, 'rev-parse', 'HEAD^{tree}'), runnerSha256: hash(overlay), fixtureSha256: amendment.fixtureSha256, bindingSha256: amendment.bindings[side] };
  }
  write(path.join(output, 'measurement.json'), { mode, expected, registeredProtocol: hash(fs.readFileSync(path.join(__dirname, 'protocol.json'))), amendmentSha256: hash(fs.readFileSync(path.join(__dirname, 'protocol-amendment.json'))), scriptSha256: hash(fs.readFileSync(__filename)), runnerSha256: hash(overlay), startedAt: new Date().toISOString() });
  await preconditions(output);
  const arms = mode === 'diagnose' ? ['original-passive', 'original-observed', 'isolated-passive', 'isolated-observed'] : mode === 'native' ? ['native'] : ['original-passive'];
  assert.ok(mode !== 'native' || referenceContracts, 'Native control requires ordinary output contracts');
  const pairs = [], contracts = referenceContracts ? read(referenceContracts) : {};
  for (let i = 1; i <= (mode === 'confirm' ? 40 : 4); i++) {
    for (const arm of i % 2 ? arms : [...arms].reverse()) {
      const pair = { number: i, arm, order: i % 2 ? 'AB' : 'BA' };
      for (const side of pair.order.toLowerCase()) {
        const dir = path.join(output, `${arm}-${i}-${side}`);
        pair[side] = child(side === 'a' ? base : candidate, dir, arm);
        assertIdentity(pair[side], expected[side]);
        for (const suite of pair[side].samples) {
          assertContract(suite, contracts, arm.startsWith('isolated'));
        }
        write(path.join(output, 'contracts.json'), contracts);
        console.log(JSON.stringify({ arm, pair: i, side, firstComplex: pair[side].samples[0].timings['Complex Pipeline'] }));
      }
      assert.equal(pair.a.runnerSha256, pair.b.runnerSha256);
      assert.deepEqual(pair.a.runtime.sharpVersions, pair.b.runtime.sharpVersions);
      pairs.push(pair);
    }
    if (mode === 'confirm' && (i === 20 || i === 40)) {
      const summary = harness.analyzePairs(pairs);
      write(path.join(output, 'summary.json'), { ...summary, outputContract: 'PASS', pairOrder: pairs.map(p => p.order) });
      if (i === 20 && summary.verdict === 'INCONCLUSIVE') continue;
      if (summary.verdict !== 'PASS') process.exitCode = 1;
      break;
    }
  }
  if (mode !== 'confirm') write(path.join(output, 'summary.json'), diagnosisSummary(pairs));
  write(path.join(output, 'completed.json'), { completedAt: new Date().toISOString(), processes: pairs.length * 2, host: hostState() });
}

if (require.main === module) {
  const [mode, a, b, c, contracts] = process.argv.slice(2);
  const task = mode === 'sample' ? observedSample(a, b, c)
    : ['diagnose', 'native', 'confirm'].includes(mode) && a && b && c ? measure(mode, a, b, c, contracts)
      : Promise.reject(Error('Usage: measure.cjs diagnose|native|confirm <base checkout> <candidate checkout> <new output directory>'));
  task.catch(e => { console.error(e); process.exitCode = 1; });
}

module.exports = { assertIdentity, assertContract, nativeTiming, diagnosisSummary };
