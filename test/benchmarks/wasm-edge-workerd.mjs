import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:net';
import fs from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { fileURLToPath, pathToFileURL } from 'node:url';
import zlib from 'node:zlib';

const sourceDir = path.dirname(fileURLToPath(import.meta.url));
const compatibilityDate = '2026-09-24';
const moduleKeys = {
  'mozjpeg_dec.wasm': 'jpegDecode',
  'mozjpeg_enc.wasm': 'jpegEncode',
  'squoosh_png_bg.wasm': 'pngDecode',
  'squoosh_resize_bg.wasm': 'resize',
  'webp_enc.wasm': 'webpEncode',
};
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const median = (values) => {
  const sorted = [...values].sort((a, b) => a - b);
  return (sorted[0] + sorted[sorted.length - 1]) / 2;
};

function blocked(message) {
  return Object.assign(new Error(message), { verdict: 'BLOCKED' });
}

async function freePort() {
  const server = createServer();
  await new Promise((resolve, reject) => server.once('error', reject).listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise((resolve) => server.close(resolve));
  return port;
}

async function buildEdge(directory, codecFiles, packageInfo) {
  const entry = path.join(directory, 'edge-entry.mjs');
  const bundlePath = path.join(directory, 'edge-worker.mjs');
  await fs.copyFile(path.join(sourceDir, 'wasm-edge-worker.mjs'), entry);
  const { build } = await import(pathToFileURL(path.join(directory, 'node_modules/esbuild/lib/main.js')).href);
  const buildResult = await build({ absWorkingDir: directory, entryPoints: [entry], outfile: bundlePath,
    bundle: true, format: 'esm', platform: 'browser', target: 'es2022', external: ['*.wasm'],
    metafile: true, logLevel: 'warning' });
  const inputPaths = Object.keys(buildResult.metafile.inputs);
  assert(inputPaths.some((input) => input.includes('node_modules/@alberteinshutoin/lazy-image-wasm/edge.js')),
    'published /edge entrypoint missing from bundle');
  const isolatedRoot = await fs.realpath(directory);
  const bundleInputs = [];
  for (const input of inputPaths) {
    const fullPath = await fs.realpath(path.resolve(directory, input));
    assert(fullPath.startsWith(isolatedRoot + path.sep), `workspace input entered Edge bundle: ${input}`);
    bundleInputs.push({ path: path.relative(isolatedRoot, fullPath), sha256: sha256(await fs.readFile(fullPath)) });
  }
  assert.equal(await fs.realpath(path.join(packageInfo.packageDir, 'edge.js')),
    await fs.realpath(path.join(directory, 'node_modules/@alberteinshutoin/lazy-image-wasm/edge.js')));
  const delivery = {};
  for (const [name, [codec, relative]] of Object.entries(codecFiles)) {
    const bytes = await fs.readFile(path.join(directory, 'node_modules', codec, relative));
    await fs.writeFile(path.join(directory, name), bytes);
    delivery[name] = { moduleKey: moduleKeys[name], rawBytes: bytes.length,
      gzipBytes: zlib.gzipSync(bytes).length, sha256: sha256(bytes) };
  }
  const jsBytes = await fs.readFile(bundlePath);
  delivery['edge-worker.mjs'] = { rawBytes: jsBytes.length, gzipBytes: zlib.gzipSync(jsBytes).length,
    sha256: sha256(jsBytes) };
  return { bundleInputs, delivery, deploymentRawBytes: Object.values(delivery).reduce((n, x) => n + x.rawBytes, 0),
    deploymentGzipBytes: Object.values(delivery).reduce((n, x) => n + x.gzipBytes, 0),
    packageEdgeImport: await fs.realpath(path.join(packageInfo.packageDir, 'edge.js')) };
}

async function writeConfig(directory, port) {
  const modules = Object.keys(moduleKeys).map((name) =>
    `    (name = "./${name}", wasm = embed "${name}")`).join(',\n');
  const config = `using Workerd = import "/workerd/workerd.capnp";\n` +
    `const config :Workerd.Config = (\n` +
    `  services = [(name = "main", worker = .worker)],\n` +
    `  sockets = [(name = "http", address = "127.0.0.1:${port}", http = (), service = "main")]\n);\n` +
    `const worker :Workerd.Worker = (\n` +
    `  modules = [\n    (name = "worker", esModule = embed "edge-worker.mjs"),\n${modules}\n  ],\n` +
    `  compatibilityDate = "${compatibilityDate}"\n);\n`;
  const configPath = path.join(directory, 'edge-config.capnp');
  await fs.writeFile(configPath, config);
  return configPath;
}

async function startWorkerd(directory, workerdPath) {
  const port = await freePort();
  const configPath = await writeConfig(directory, port);
  const command = [workerdPath, 'serve', configPath, 'config'];
  const startedAt = performance.now();
  const child = spawn(command[0], command.slice(1), { cwd: directory, stdio: ['ignore', 'pipe', 'pipe'] });
  let stderr = '';
  child.stderr.on('data', (chunk) => { stderr += chunk; });
  child.stdout.on('data', () => {});
  let exitError;
  child.once('error', (error) => { exitError = error; });
  const base = `http://127.0.0.1:${port}`;
  try {
    let health;
    while (performance.now() - startedAt < 10000) {
      if (exitError) throw blocked(`workerd executable could not start: ${exitError.message}`);
      if (child.exitCode !== null) throw new Error(`workerd launch failed: ${stderr}`);
      try {
        const response = await fetch(`${base}/health`, { signal: AbortSignal.timeout(300) });
        if (!response.ok) throw new Error(`health HTTP ${response.status}`);
        health = await response.json();
        break;
      } catch {
        await new Promise((resolve) => setTimeout(resolve, 25));
      }
    }
    if (!health) throw new Error(`workerd did not become ready within 10s: ${stderr}`);
    assert.equal(health.optimizerCreations, 0, 'health check pre-ran image optimizer');
    assert.deepEqual(Object.keys(health.wasmModules).sort(), Object.values(moduleKeys).sort(),
      'workerd imported an incomplete Wasm module set');
    assert(Object.values(health.wasmModules).every(Boolean), 'workerd did not import real WebAssembly.Module values');
    return { child, base, health, command, startedAt, readyMs: performance.now() - startedAt,
      getStderr: () => stderr.trim() };
  } catch (error) {
    child.kill();
    throw error;
  }
}

async function stopWorkerd(server) {
  if (server.child.exitCode !== null) return;
  let timeout;
  const closed = new Promise((resolve) => server.child.once('close', () => {
    clearTimeout(timeout);
    resolve();
  }));
  server.child.kill();
  await Promise.race([closed, new Promise((resolve) => { timeout = setTimeout(resolve, 2000); })]);
  if (server.child.exitCode === null) server.child.kill('SIGKILL');
}

async function requestImage(server, input, options) {
  const startedAt = performance.now();
  let response;
  try {
    response = await fetch(`${server.base}/process`, { method: 'POST',
      headers: { 'x-lazy-options': JSON.stringify(options) }, body: input,
      signal: AbortSignal.timeout(30000) });
  } catch (error) {
    throw new Error(`workerd image request failed: ${error.message}; stderr: ${server.getStderr()}; exit: ${server.child.exitCode}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  const wallMs = performance.now() - startedAt;
  if (!response.ok) {
    const error = JSON.parse(bytes.toString('utf8'));
    return { status: 'FAIL', wallMs, httpStatus: response.status, error };
  }
  const result = JSON.parse(response.headers.get('x-lazy-result'));
  assert.equal(result.bytesOut, bytes.length);
  assert.equal(result.isolateId, server.health.isolateId);
  assert.equal(result.optimizerCreations, 1);
  return { status: 'PASS', wallMs, bytes, result };
}

async function runCase(directory, workerdPath, item, outputDir, validateOutput) {
  const server = await startWorkerd(directory, workerdPath);
  try {
    const input = await fs.readFile(item.input);
    const first = await requestImage(server, input, item.options);
    if (first.status !== 'PASS') throw new Error(`${item.id}: ${first.error?.code ?? first.error?.message}`);
    const coldFromBeforeRuntimeMs = performance.now() - server.startedAt;
    const extension = item.options.format === 'jpeg' ? 'jpg' : 'webp';
    const output = await validateOutput(first.bytes, first.result, item,
      path.join(outputDir, `wasm-edge-${item.id}.${extension}`));
    const warm = [await requestImage(server, input, item.options), await requestImage(server, input, item.options)];
    assert(warm.every((entry) => entry.status === 'PASS'), `${item.id}: warm image processing failed`);
    let budget = null;
    if (item.budgetCases) {
      const impossible = { ...item.options, targetBytes: 10, minQuality: 50, maxQuality: 51 };
      const bestEffort = await requestImage(server, input, { ...impossible, qualityFloorPolicy: 'best-effort' });
      assert.equal(bestEffort.status, 'PASS');
      assert.equal(bestEffort.result.budgetMet, false);
      assert(bestEffort.bytes.length > 10);
      const bestEffortOutput = await validateOutput(bestEffort.bytes, bestEffort.result, item,
        path.join(outputDir, `wasm-edge-${item.id}-best-effort.jpg`));
      const strict = await requestImage(server, input, { ...impossible, qualityFloorPolicy: 'strict' });
      assert.equal(strict.status, 'FAIL');
      assert.equal(strict.error.code, 'E502');
      assert.equal(strict.error.category, 'ResourceLimit');
      budget = { bestEffort: { bytesOut: bestEffort.bytes.length, budgetMet: false,
        wallMs: bestEffort.wallMs, output: bestEffortOutput },
        strict: { expectedRejection: true, code: strict.error.code, wallMs: strict.wallMs } };
    }
    let clock;
    try {
      clock = await (await fetch(`${server.base}/clock`, { signal: AbortSignal.timeout(3000) })).json();
    } catch (error) {
      clock = { status: 'unavailable', reason: `diagnostic request failed after image processing: ${error.message}; cause: ${error.cause?.message ?? 'unknown'}; workerd exit: ${server.child.exitCode}` };
    }
    return { id: item.id, status: 'PASS', launchCommand: server.command,
      imageDataType: server.health.imageDataType,
      coldFromBeforeRuntimeMs, startupToReadyMs: server.readyMs,
      firstRequestMs: first.wallMs, warmMs: warm.map((entry) => entry.wallMs),
      warmMedianMs: median(warm.map((entry) => entry.wallMs)),
      isolateId: server.health.isolateId, optimizerCreations: 1, clock, metrics: first.result.metrics,
      output, budget, workerdStderr: server.getStderr() };
  } finally {
    await stopWorkerd(server);
  }
}

export async function runEdgeWorkerd({ directory, packageInfo, cases, codecFiles, outputDir,
  validateOutput, workerdPath }) {
  const report = { status: 'FAIL', runtime: 'local workerd', compatibilityDate,
    compatibilityFlags: [], workerdCommand: null, results: [], probe: null };
  try {
    const binary = workerdPath ?? path.join(directory, 'node_modules/.bin/workerd');
    try { await fs.access(binary); } catch { throw blocked(`workerd executable unavailable: ${binary}`); }
    const binarySha256 = sha256(await fs.readFile(binary));
    const expectedSha256 = packageInfo.toolchainBinaries?.workerd?.sha256;
    if (!expectedSha256 || binarySha256 !== expectedSha256) {
      throw new Error(`workerd executable differs from installed registry package: ${binary}`);
    }
    report.executedWorkerdBinary = { path: await fs.realpath(binary), sha256: binarySha256,
      package: packageInfo.toolchainBinaries.workerd.package };
    const version = spawnSync(binary, ['--version'], { encoding: 'utf8' });
    if (version.error || version.status !== 0) throw blocked(`workerd --version failed: ${version.error?.message ?? version.stderr}`);
    report.workerdVersion = version.stdout.trim();
    report.workerdPackageVersion = JSON.parse(await fs.readFile(path.join(directory, 'node_modules/workerd/package.json'))).version;
    report.esbuildVersion = JSON.parse(await fs.readFile(path.join(directory, 'node_modules/esbuild/package.json'))).version;
    report.workerdCommand = `${binary} serve <isolated-install>/edge-config.capnp config`;
    const bundle = await buildEdge(directory, codecFiles, packageInfo);
    Object.assign(report, bundle);
    const probe = { id: 'probe-jpeg-webp', input: path.resolve(sourceDir, '../fixtures/test_100KB_1057x1057.jpg'),
      options: { format: 'webp', maxWidth: 320, maxHeight: 320, targetBytes: 50000,
        minQuality: 45, maxQuality: 86, qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' } };
    report.probeInput = { path: path.relative(path.resolve(sourceDir, '../..'), probe.input),
      sha256: sha256(await fs.readFile(probe.input)), options: probe.options };
    report.probe = await runCase(directory, binary, probe, outputDir, validateOutput);
    for (const item of cases) {
      try {
        report.results.push(await runCase(directory, binary, item, outputDir, validateOutput));
      } catch (error) {
        report.results.push({ id: item.id, status: 'FAIL', reason: error.message });
        throw error;
      }
    }
    report.coldCondition = 'new workerd process per case; health check does not create optimizer; two warm requests reuse isolate and optimizer';
    report.loadMode = 'static workerd WebAssembly.Module imports passed to published /edge wasmModules';
    report.status = 'PASS';
    return report;
  } catch (error) {
    report.status = error.verdict ?? 'FAIL';
    report.error = { message: error.message, stack: error.stack };
    error.partialEdgeResults = report;
    throw error;
  }
}
