'use strict';

const fs = require('node:fs');
const path = require('node:path');
const os = require('node:os');
const crypto = require('node:crypto');
const cp = require('node:child_process');
const assert = require('node:assert/strict');
const { createRequire } = require('node:module');
const { performance } = require('node:perf_hooks');

const CASES = ['JPEG', 'WebP', 'AVIF', 'Complex Pipeline'];
const TARGETS = ['JPEG', 'AVIF', 'Complex Pipeline'];
const PHASES = ['first', 'warm1', 'warm2', 'warm3'];
const RUNNER = 'test/benchmarks/sharp-comparison.bench.js';
const LIMIT = 1.15;
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = file => JSON.parse(fs.readFileSync(file, 'utf8'));
const write = (file, value) => fs.writeFileSync(file, JSON.stringify(value, null, 2) + '\n');
const git = (root, ...args) => cp.execFileSync('git', args, { cwd: root, encoding: 'utf8' }).trim();
const median = values => { const s = [...values].sort((a, b) => a - b); return (s[Math.floor((s.length - 1) / 2)] + s[Math.floor(s.length / 2)]) / 2; };

function interval(ratios) {
  let seed = 913;
  const values = [];
  for (let b = 0; b < 20000; b++) {
    const sample = [];
    for (let i = 0; i < ratios.length; i++) {
      seed ^= seed << 13; seed ^= seed >>> 17; seed ^= seed << 5;
      sample.push(ratios[(seed >>> 0) % ratios.length]);
    }
    values.push(median(sample));
  }
  values.sort((a, b) => a - b);
  return [values[499], values[19499]];
}

function analyzePairs(pairs) {
  assert.ok(pairs.length >= 5 && pairs.length <= 40, 'Expected five to forty independent pairs');
  const rows = [];
  for (const name of CASES) for (const engine of ['lazy', 'sharp']) for (const phase of ['first', 'warm']) {
    const times = { a: [], b: [] };
    for (const pair of pairs) for (const side of ['a', 'b']) {
      const samples = pair[side].samples.filter(s => phase === 'first' ? s.phase === 'first' : s.phase.startsWith('warm'));
      assert.equal(samples.length, phase === 'first' ? 1 : 3);
      const values = samples.map(s => s.timings[name][engine]);
      assert.ok(values.every(v => Number.isFinite(v) && v > 0), 'Missing or invalid timing');
      times[side].push(median(values));
    }
    const ratios = times.b.map((v, i) => v / times.a[i]);
    const range = interval(ratios), center = median(ratios);
    rows.push({ name, engine, phase, pairs: pairs.length, times, ratios, medianRatio: center,
      medianChangePercent: (center - 1) * 100, minRatio: Math.min(...ratios), maxRatio: Math.max(...ratios),
      madRatio: median(ratios.map(r => Math.abs(r - center))), interval95: range,
      status: range[1] <= LIMIT ? 'pass' : range[0] > LIMIT ? 'regression' : 'inconclusive' });
  }
  const targets = rows.filter(r => r.engine === 'lazy' && TARGETS.includes(r.name));
  return { thresholdRatio: LIMIT, bootstrap: { seed: 913, resamples: 20000, interval: 'percentile 95%; exploratory small-sample uncertainty' },
    verdict: targets.some(r => r.status === 'regression') ? 'REGRESSION' : targets.every(r => r.status === 'pass') ? 'PASS' : 'INCONCLUSIVE', rows };
}

function optionalFile(file) { return fs.existsSync(file) ? fs.readFileSync(file, 'utf8').trim() : null; }

async function sample(root, output, { isolated = false } = {}) {
  fs.mkdirSync(output);
  const report = { startedAt: new Date().toISOString(), samples: [] };
  try {
    // Only the identical measurement overlay may differ from each reviewed revision.
    const protectedPaths = ['src', 'Cargo.toml', 'Cargo.lock', 'package.json', 'package-lock.json', 'build.rs', 'scripts/build-native.cjs', 'scripts/postbuild-fixup.js', 'index.js', 'index.d.ts', 'test/helpers/quality.js', 'test/helpers/paths.js'];
    cp.execFileSync('git', ['diff', '--quiet', 'HEAD', '--', ...protectedPaths], { cwd: root });
    const local = createRequire(path.join(root, 'package.json'));
    const loadStart = performance.now();
    const { runBenchmark } = local('./' + RUNNER);
    const sharp = local('sharp');
    const fixture = path.join(root, 'test/fixtures/test_4.5MB_5000x5000.png');
    const sourceFiles = git(root, 'ls-files', '--', ...protectedPaths).split('\n').filter(Boolean);
    const sourceHashes = Object.fromEntries(sourceFiles.map(f => [f, hash(fs.readFileSync(path.join(root, f)))]));
    const bindings = Object.keys(require.cache).filter(f => f.startsWith(root + path.sep) && f.endsWith('.node') && !f.includes(path.sep + 'node_modules' + path.sep));
    assert.equal(bindings.length, 1, 'Expected the native binding inside this checkout');
    assert.equal(fs.realpathSync(path.dirname(bindings[0])), fs.realpathSync(root));
    Object.assign(report, { revision: git(root, 'rev-parse', 'HEAD'), sourceTree: git(root, 'rev-parse', 'HEAD^{tree}'),
      sourceHashes, sourceHashesSha256: hash(JSON.stringify(sourceHashes)), dirtyPaths: git(root, 'status', '--porcelain'),
      modifiedPaths: git(root, 'diff', '--name-only', '-z', 'HEAD').split('\0').filter(Boolean),
      untrackedPaths: git(root, 'ls-files', '--others', '--exclude-standard', '-z').split('\0').filter(Boolean),
      runnerSha256: hash(fs.readFileSync(path.join(root, RUNNER))), harnessSha256: hash(fs.readFileSync(__filename)),
      binding: { path: bindings[0], sha256: hash(fs.readFileSync(bindings[0])) },
      fixture: { path: fixture, sha256: hash(fs.readFileSync(fixture)) },
      runtime: { node: process.version, executable: process.execPath, os: os.type(), release: os.release(), version: os.version(), arch: os.arch(), cpus: os.cpus().map(c => ({ model: c.model, speed: c.speed })), parallelism: os.availableParallelism(), sharpVersions: sharp.versions, sharpConcurrency: sharp.concurrency(), sharpCache: sharp.cache(),
        env: Object.fromEntries(['UV_THREADPOOL_SIZE', 'RAYON_NUM_THREADS', 'RUSTUP_TOOLCHAIN', 'SDKROOT', 'RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CFLAGS', 'CXXFLAGS', 'MACOSX_DEPLOYMENT_TARGET', 'ImageOS', 'ImageVersion', 'RUNNER_OS', 'RUNNER_ARCH', 'RUNNER_NAME'].map(k => [k, process.env[k] ?? null])), cpuQuota: optionalFile('/sys/fs/cgroup/cpu.max'), cpuStatBefore: optionalFile('/sys/fs/cgroup/cpu.stat'), loadBefore: os.loadavg() },
      setupMs: performance.now() - loadStart,
      isolated,
      timingScope: 'Canonical Date.now operation timers; first after module/fixture/reference preparation, warm median of three subsequent suites; not process startup or OS-cold I/O' });
    fs.mkdirSync(path.join(output, 'objects'));
    for (const phase of PHASES) {
      const outputs = [];
      const started = performance.now();
      await runBenchmark({ ...(isolated ? { cases: [] } : {}), metricsOutputPath: path.join(output, phase + '-metrics.json'), qualityOutputPath: path.join(output, phase + '-quality.json'),
        onOutput: async ({ name, ...buffers }) => {
          for (const [engine, buffer] of Object.entries(buffers)) {
            assert.ok(Buffer.isBuffer(buffer));
            const metadata = await sharp(buffer).metadata(), digest = hash(buffer);
            const object = 'objects/' + digest;
            if (!fs.existsSync(path.join(output, object))) fs.writeFileSync(path.join(output, object), buffer);
            outputs.push({ name, engine, bytes: buffer.length, sha256: digest, object,
              metadata: Object.fromEntries(['format', 'compression', 'width', 'height', 'space', 'channels', 'hasAlpha', 'orientation'].map(k => [k, metadata[k] ?? null])),
              exifSha256: metadata.exif ? hash(metadata.exif) : null, iccSha256: metadata.icc ? hash(metadata.icc) : null, xmpSha256: metadata.xmp ? hash(metadata.xmp) : null });
          }
        } });
      const quality = read(path.join(output, phase + '-quality.json'));
      assert.equal(outputs.length, isolated ? 2 : 8, 'Every selected output from both engines must be observed');
      const cases = [...quality.results, ...quality.performanceOnly];
      assert.deepEqual(cases.map(r => r.name), isolated ? ['Complex Pipeline'] : CASES);
      report.samples.push({ phase, suiteWallMs: performance.now() - started,
        timings: Object.fromEntries(cases.map(r => [r.name, { lazy: r.lazy.time, sharp: r.sharp.time }])),
        qualityAxes: quality.results.map(r => ({ name: r.name, status: r.status, axes: r.axes })), outputs });
      write(path.join(output, 'sample.json'), report);
    }
    report.runtime.cpuStatAfter = optionalFile('/sys/fs/cgroup/cpu.stat');
    report.runtime.loadAfter = os.loadavg();
    report.status = 'PASS';
  } catch (error) {
    report.status = 'FAIL'; report.error = { message: error.message, stack: error.stack };
    throw error;
  } finally {
    report.finishedAt = new Date().toISOString(); write(path.join(output, 'sample.json'), report);
  }
}

function compare(base, candidate, output) {
  assert.notEqual(fs.realpathSync(base), fs.realpathSync(candidate));
  assert.ok(!fs.existsSync(output), 'Do not overwrite a previous measurement');
  fs.mkdirSync(output, { recursive: true });
  const runner = fs.readFileSync(path.resolve(__dirname, '../../', RUNNER));
  const overlay = [];
  for (const root of [base, candidate]) {
    const file = path.join(root, RUNNER);
    overlay.push({ root, revision: git(root, 'rev-parse', 'HEAD'), originalRunnerSha256: hash(fs.readFileSync(file)), measurementRunnerSha256: hash(runner) });
    fs.writeFileSync(file, runner);
  }
  write(path.join(output, 'overlay.json'), overlay);
  const pairs = [];
  let firstContract;
  for (let i = 1; i <= 10; i++) {
    const pair = { number: i, order: i % 2 ? 'AB' : 'BA' };
    for (const side of pair.order.toLowerCase()) {
      const root = side === 'a' ? base : candidate, dir = path.join(output, `${i}-${side}`);
      const fd = fs.openSync(dir + '.log', 'w');
      const result = cp.spawnSync(process.execPath, [__filename, '--sample', root, dir], { cwd: root, env: { ...process.env, UV_THREADPOOL_SIZE: '4' }, stdio: ['ignore', fd, fd] });
      fs.closeSync(fd);
      assert.equal(result.status, 0, `Sample ${i}-${side} failed; retain its log and partial report`);
      pair[side] = read(path.join(dir, 'sample.json'));
      assert.equal(pair[side].status, 'PASS');
      for (const suite of pair[side].samples) {
        const contract = JSON.stringify({ outputs: suite.outputs, qualityAxes: suite.qualityAxes });
        if (firstContract === undefined) firstContract = contract;
        assert.equal(contract, firstContract, 'Output hashes/metadata/quality contract differs; timing cannot pass');
      }
    }
    assert.equal(pair.a.fixture.sha256, pair.b.fixture.sha256);
    assert.equal(pair.a.runnerSha256, pair.b.runnerSha256);
    assert.deepEqual(pair.a.runtime.sharpVersions, pair.b.runtime.sharpVersions);
    pairs.push(pair);
    if (i < 5) continue;
    const summary = analyzePairs(pairs);
    write(path.join(output, 'summary.json'), { ...summary, outputContract: 'PASS', pairOrder: pairs.map(p => p.order), overlay,
      processReports: pairs.flatMap(p => ['a', 'b'].map(s => ({ pair: p.number, side: s, path: `${p.number}-${s}/sample.json`, sha256: hash(fs.readFileSync(path.join(output, `${p.number}-${s}/sample.json`))) }))) });
    console.log(JSON.stringify({ pairs: i, verdict: summary.verdict }));
    if (i === 5 && summary.verdict === 'INCONCLUSIVE') continue;
    if (i > 5 && i < 10) continue;
    if (summary.verdict !== 'PASS') process.exitCode = 1;
    return;
  }
}

if (require.main === module) {
  const args = process.argv.slice(2);
  const task = args[0] === '--sample' && args.length === 3
    ? sample(path.resolve(args[1]), path.resolve(args[2]))
    : args.length === 6 && args[0] === '--base' && args[2] === '--candidate' && args[4] === '--output'
      ? Promise.resolve().then(() => compare(path.resolve(args[1]), path.resolve(args[3]), path.resolve(args[5])))
      : Promise.reject(new Error('Usage: paired-comparison.cjs --base <checkout> --candidate <checkout> --output <new directory>'));
  task.catch(error => { console.error(error); process.exitCode = 1; });
}

module.exports = { analyzePairs, sample };
