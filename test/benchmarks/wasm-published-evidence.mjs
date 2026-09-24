import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { createServer } from 'node:http';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import zlib from 'node:zlib';
import sharp from 'sharp';

const sourceDir = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(sourceDir, '../..');
const outputDir = path.join(root, 'artifacts/benchmark');
const packageName = '@alberteinshutoin/lazy-image-wasm';
const registry = 'https://registry.npmjs.org/';
const codecFiles = {
  'mozjpeg_dec.wasm': ['@jsquash/jpeg', 'codec/dec/mozjpeg_dec.wasm'],
  'mozjpeg_enc.wasm': ['@jsquash/jpeg', 'codec/enc/mozjpeg_enc.wasm'],
  'squoosh_png_bg.wasm': ['@jsquash/png', 'codec/pkg/squoosh_png_bg.wasm'],
  'squoosh_resize_bg.wasm': ['@jsquash/resize', 'lib/resize/pkg/squoosh_resize_bg.wasm'],
  'webp_enc.wasm': ['@jsquash/webp', 'codec/enc/webp_enc.wasm'],
};
const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
const median = (numbers) => {
  const sorted = [...numbers].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2 ? sorted[middle] : (sorted[middle - 1] + sorted[middle]) / 2;
};
const summarize = (results) => ({
  imageConversionsPassed: results.length + results.filter((entry) => entry.budget?.bestEffort).length,
  imageConversionsAttempted: results.length + results.filter((entry) => entry.budget).length,
  budgetMet: results.filter((entry) => entry.metrics.budgetMet).length,
  budgetAttempts: results.length + results.filter((entry) => entry.budget).length,
  expectedStrictRejections: results.filter((entry) => entry.budget?.strict.expectedRejection).length,
  strictAttempts: results.filter((entry) => entry.budget).length,
});

function command(file, args, cwd) {
  const result = spawnSync(file, args, { cwd, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  if (result.error || result.status !== 0) {
    throw new Error(`${file} ${args.join(' ')} failed: ${result.error?.message ?? result.stderr}`);
  }
  return result.stdout.trim();
}

async function installPublished(version, directory) {
  await fs.writeFile(path.join(directory, 'package.json'), JSON.stringify({ private: true, type: 'module' }));
  command('npm', ['install', '--registry', registry, '--save-exact', `${packageName}@${version}`, 'esbuild@0.25.10'], directory);
  const lock = JSON.parse(await fs.readFile(path.join(directory, 'package-lock.json'), 'utf8'));
  const names = [packageName, '@jsquash/jpeg', '@jsquash/png', '@jsquash/resize', '@jsquash/webp'];
  const packages = Object.fromEntries(names.map((name) => {
    const entry = lock.packages[`node_modules/${name}`];
    assert(entry?.resolved?.startsWith(registry) && entry.integrity, `registry provenance missing: ${name}`);
    return [name, { version: entry.version, resolved: entry.resolved, integrity: entry.integrity }];
  }));
  assert.equal(packages[packageName].version, version);
  const packageDir = path.join(directory, 'node_modules', packageName);
  const browserImport = await fs.realpath(path.join(packageDir, 'browser.js'));
  assert(browserImport.startsWith((await fs.realpath(directory)) + path.sep));
  return { packages, packageDir, browserImport };
}

async function prepareCases(directory) {
  const metadataInput = path.join(directory, 'metadata-exif-gps-xmp.jpg');
  const xmp = '<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about=""/></rdf:RDF></x:xmpmeta>';
  await sharp(path.join(root, 'test/benchmarks/corpus/images/metadata-1.jpg'))
    .keepMetadata().withXmp(xmp).jpeg().toFile(metadataInput);
  await fs.copyFile(metadataInput, path.join(outputDir, 'wasm-metadata-input.jpg'));
  const cases = [
    { id: 'jpeg-webp', input: path.join(root, 'test/fixtures/test_3.2MB_5000x5000.jpg'),
      options: { format: 'webp', maxWidth: 1600, maxHeight: 1600, targetBytes: 500000,
        minQuality: 45, maxQuality: 86, qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' } },
    { id: 'png-jpeg', input: path.join(root, 'test/fixtures/test_4.5MB_5000x5000.png'),
      options: { format: 'jpeg', maxWidth: 1600, maxHeight: 1600, targetBytes: 450000,
        minQuality: 45, maxQuality: 90, qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' } },
    { id: 'metadata-budget', input: metadataInput, metadataCase: true, budgetCases: true,
      options: { format: 'jpeg', maxWidth: 320, maxHeight: 240, targetBytes: 30000,
        minQuality: 45, maxQuality: 86, qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' } },
  ];
  for (const item of cases) {
    const bytes = await fs.readFile(item.input);
    item.inputBytes = bytes.length;
    item.inputSha256 = sha256(bytes);
    const meta = await sharp(bytes).metadata();
    item.inputMetadata = { exif: Boolean(meta.exif), gpsTag: Boolean(meta.exif?.includes(Buffer.from([0x25, 0x88]))),
      xmp: Boolean(meta.xmp), icc: Boolean(meta.icc) };
    if (item.metadataCase) assert(item.inputMetadata.exif && item.inputMetadata.gpsTag && item.inputMetadata.xmp);
  }
  return cases;
}

async function validateOutput(bytes, result, item, outputPath) {
  const meta = await sharp(bytes).metadata();
  await sharp(bytes).raw().toBuffer(); // Decode independently of the package under test.
  assert.equal(meta.format, item.options.format);
  assert.equal(meta.width, result.metrics.widthOut);
  assert.equal(meta.height, result.metrics.heightOut);
  assert.equal(bytes.length, result.bytesOut);
  assert.equal(bytes.length, result.metrics.bytesOut);
  assert.equal(result.budgetMet, bytes.length <= result.targetBytes);
  assert.equal(result.metrics.budgetMet, result.budgetMet);
  assert(meta.width <= item.options.maxWidth && meta.height <= item.options.maxHeight);
  const metadata = { exif: Boolean(meta.exif), xmp: Boolean(meta.xmp), icc: Boolean(meta.icc) };
  if (item.metadataCase) assert(!metadata.exif && !metadata.xmp && !metadata.icc, 'output retained input metadata');
  await fs.writeFile(outputPath, bytes);
  return { sha256: sha256(bytes), bytes: bytes.length, format: meta.format,
    width: meta.width, height: meta.height, metadata, artifact: path.relative(root, outputPath) };
}

async function runNode(packageInfo, directory, cases) {
  // jSquash needs ImageData in Node; this does not emulate a browser or Edge isolate.
  if (typeof globalThis.ImageData !== 'function') {
    globalThis.ImageData = class ImageData {
      constructor(data, width, height) { this.data = data; this.width = width; this.height = height; }
    };
  }
  const { createUploadOptimizer } = await import(pathToFileURL(packageInfo.browserImport).href);
  const modules = {};
  for (const [name, [codec, relative]] of Object.entries(codecFiles)) {
    const key = { 'mozjpeg_dec.wasm': 'jpegDecode', 'mozjpeg_enc.wasm': 'jpegEncode',
      'squoosh_png_bg.wasm': 'pngDecode', 'squoosh_resize_bg.wasm': 'resize', 'webp_enc.wasm': 'webpEncode' }[name];
    modules[key] = await fs.readFile(path.join(directory, 'node_modules', codec, relative));
  }
  const creationStart = performance.now();
  const optimizer = await createUploadOptimizer({ wasmModules: modules, defaultOutput: 'arrayBuffer' });
  const optimizerCreationMs = performance.now() - creationStart;
  const results = [];
  for (const item of cases) {
    const input = await fs.readFile(item.input);
    const run = async (options) => {
      const start = performance.now();
      try {
        const result = await optimizer.optimizeUpload(input, options);
        return { status: 'PASS', wallMs: performance.now() - start, result };
      } catch (error) {
        return { status: 'FAIL', wallMs: performance.now() - start,
          error: { code: error.code, category: error.category, message: error.message } };
      }
    };
    const first = await run(item.options);
    assert.equal(first.status, 'PASS', `${item.id}: ${first.error?.message}`);
    const output = await validateOutput(Buffer.from(first.result.data), first.result, item,
      path.join(outputDir, `wasm-node-${item.id}.${item.options.format === 'jpeg' ? 'jpg' : 'webp'}`));
    const warm = [await run(item.options), await run(item.options)];
    assert(warm.every((entry) => entry.status === 'PASS'));
    let budget = null;
    if (item.budgetCases) {
      const impossible = { ...item.options, targetBytes: 10, minQuality: 50, maxQuality: 51 };
      const bestEffort = await run({ ...impossible, qualityFloorPolicy: 'best-effort' });
      const strict = await run({ ...impossible, qualityFloorPolicy: 'strict' });
      assert.equal(bestEffort.status, 'PASS');
      assert.equal(bestEffort.result.budgetMet, false);
      assert(bestEffort.result.bytesOut > 10);
      assert.equal(strict.error?.code, 'E502');
      assert.equal(strict.error?.category, 'ResourceLimit');
      const bestEffortOutput = await validateOutput(Buffer.from(bestEffort.result.data), bestEffort.result, item,
        path.join(outputDir, `wasm-node-${item.id}-best-effort.jpg`));
      budget = { bestEffort: { bytesOut: bestEffort.result.bytesOut, budgetMet: false,
        wallMs: bestEffort.wallMs, output: bestEffortOutput },
        strict: { expectedRejection: true, code: strict.error.code, wallMs: strict.wallMs } };
    }
    results.push({ id: item.id, status: 'PASS', optimizerCreationMs, firstMs: first.wallMs,
      warmMs: warm.map((entry) => entry.wallMs), warmMedianMs: median(warm.map((entry) => entry.wallMs)),
      metrics: first.result.metrics, output, budget });
  }
  return results;
}

async function bundleBrowser(directory, packageDir) {
  const browserDir = path.join(directory, 'browser');
  await fs.mkdir(path.join(browserDir, 'assets'), { recursive: true });
  const { build } = await import(pathToFileURL(path.join(directory, 'node_modules/esbuild/lib/main.js')).href);
  const workerPath = path.join(browserDir, 'worker.js');
  const isolatedEntry = path.join(directory, 'worker-entry.mjs');
  await fs.copyFile(path.join(sourceDir, 'wasm-browser-worker.mjs'), isolatedEntry);
  const buildResult = await build({ entryPoints: [isolatedEntry], outfile: workerPath,
    bundle: true, format: 'esm', platform: 'browser', target: 'es2022',
    metafile: true, logLevel: 'warning' });
  const bundleInputs = Object.keys(buildResult.metafile.inputs);
  assert(bundleInputs.some((input) => input.includes('node_modules/@alberteinshutoin/lazy-image-wasm/browser.js')));
  assert(bundleInputs.some((input) => input.includes('node_modules/@alberteinshutoin/lazy-image-wasm/worker.js')));
  const isolatedRoot = await fs.realpath(directory);
  assert(bundleInputs.every((input) => path.resolve(root, input).startsWith(isolatedRoot + path.sep)),
    'workspace module entered browser bundle');
  await fs.copyFile(path.join(sourceDir, 'wasm-browser-main.mjs'), path.join(browserDir, 'main.js'));
  const assets = {};
  for (const [name, [codec, relative]] of Object.entries(codecFiles)) {
    const target = path.join(browserDir, 'assets', name);
    await fs.copyFile(path.join(directory, 'node_modules', codec, relative), target);
    assets[`/assets/${name}`] = target;
  }
  const directoryBytes = async (dir) => {
    let total = 0;
    for (const entry of await fs.readdir(dir, { withFileTypes: true })) {
      const item = path.join(dir, entry.name);
      total += entry.isDirectory() ? await directoryBytes(item) : (await fs.stat(item)).size;
    }
    return total;
  };
  const packageDirectoryBytes = await directoryBytes(packageDir);
  const delivery = {};
  for (const [url, file] of Object.entries({ '/main.js': path.join(browserDir, 'main.js'),
    '/worker.js': workerPath, ...assets })) {
    const bytes = await fs.readFile(file);
    delivery[url] = { rawBytes: bytes.length, gzipBytes: zlib.gzipSync(bytes).length, sha256: sha256(bytes) };
  }
  return { browserDir, assets, delivery, packageDirectoryBytes,
    bundlePackageInputs: bundleInputs.filter((input) => input.includes('node_modules/@alberteinshutoin/lazy-image-wasm/'))
      .map((input) => path.resolve(root, input)) };
}

async function runBrowser(directory, cases, bundle, chromePath) {
  const chromeVersion = command(chromePath, ['--version'], directory);
  const requests = [];
  let receiveReport;
  const reportPromise = new Promise((resolve) => { receiveReport = resolve; });
  const config = cases.map(({ id, options, budgetCases }) => ({ id, options, budgetCases: Boolean(budgetCases) }));
  const inputs = Object.fromEntries(cases.map((item) => [item.id, item.input]));
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, 'http://localhost').pathname;
      if (url === '/report' && request.method === 'POST') {
        const chunks = [];
        for await (const chunk of request) chunks.push(chunk);
        const report = JSON.parse(Buffer.concat(chunks).toString('utf8'));
        receiveReport(report);
        response.writeHead(200); response.end('ok');
        return;
      }
      let body;
      let type;
      if (url === '/') { body = Buffer.from('<!doctype html><meta charset="utf-8"><pre id="status">Running</pre><script type="module" src="/main.js"></script>'); type = 'text/html'; }
      else if (url === '/config.json') { body = Buffer.from(JSON.stringify(config)); type = 'application/json'; }
      else {
        const file = url.startsWith('/input/') ? inputs[url.slice('/input/'.length)] :
          url === '/main.js' || url === '/worker.js' ? path.join(bundle.browserDir, url.slice(1)) : bundle.assets[url];
        if (!file) { response.writeHead(404); response.end(); return; }
        body = await fs.readFile(file);
        type = url.endsWith('.wasm') ? 'application/wasm' : url.endsWith('.js') ? 'text/javascript' : 'image/jpeg';
      }
      const gzip = /\bgzip\b/.test(request.headers['accept-encoding'] ?? '');
      const transmitted = gzip ? zlib.gzipSync(body) : body;
      response.writeHead(200, { 'content-type': type, 'cache-control': 'no-store',
        ...(gzip ? { 'content-encoding': 'gzip' } : {}), 'content-length': transmitted.length });
      response.end(transmitted);
      requests.push({ url, rawBytes: body.length, transferredBodyBytes: transmitted.length, gzip });
    } catch (error) { response.writeHead(500); response.end(error.stack); receiveReport({ error: { message: error.message } }); }
  });
  await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
  const address = server.address();
  const profile = path.join(directory, 'chrome-profile');
  const chrome = spawn(chromePath, ['--headless=new', '--no-first-run', '--disable-background-networking',
    `--user-data-dir=${profile}`, `http://127.0.0.1:${address.port}/`], { stdio: ['ignore', 'ignore', 'pipe'] });
  const chromeClosed = new Promise((resolve) => chrome.once('close', resolve));
  let stderr = '';
  chrome.stderr.on('data', (chunk) => { stderr = (stderr + chunk.toString()).slice(-10000); });
  const chromeFailure = new Promise((_, reject) => {
    chrome.on('error', (error) => reject(new Error(`Chrome failed to start: ${error.message}`)));
    chrome.on('exit', (code) => reject(new Error(`Chrome exited before reporting (code ${code}): ${stderr}`)));
  });
  try {
    let timeout;
    const browserReport = await Promise.race([reportPromise, chromeFailure,
      new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error(`Chrome report timeout: ${stderr}`)), 300000); })])
      .finally(() => clearTimeout(timeout));
    assert(!browserReport.error, browserReport.error?.stack ?? browserReport.error?.message);
    assert(browserReport.userAgent.includes('Chrome/'));
    const results = [];
    for (const item of cases) {
      const entry = browserReport.cases.find((candidate) => candidate.id === item.id);
      assert(entry, `missing browser case: ${item.id}`);
      assert(entry.cold.ok, `${item.id}: ${entry.cold.error?.stack ?? entry.cold.error?.message}`);
      assert(entry.warm.every((run) => run.ok));
      const outputBytes = Buffer.from(entry.cold.result.data);
      const output = await validateOutput(outputBytes, entry.cold.result, item,
        path.join(outputDir, `wasm-browser-${item.id}.${item.options.format === 'jpeg' ? 'jpg' : 'webp'}`));
      let budget = null;
      if (item.budgetCases) {
        assert(entry.bestEffort.ok);
        assert.equal(entry.bestEffort.result.budgetMet, false);
        assert(entry.bestEffort.result.bytesOut > 10);
        assert.equal(entry.strict.error?.code, 'E502');
        assert.equal(entry.strict.error?.category, 'ResourceLimit');
        const bestEffortOutput = await validateOutput(Buffer.from(entry.bestEffort.result.data),
          entry.bestEffort.result, item, path.join(outputDir, `wasm-browser-${item.id}-best-effort.jpg`));
        budget = { bestEffort: { bytesOut: entry.bestEffort.result.bytesOut, budgetMet: false,
          wallMs: entry.bestEffort.wallMs, output: bestEffortOutput },
          strict: { expectedRejection: true, code: 'E502', wallMs: entry.strict.wallMs } };
      }
      results.push({ id: item.id, status: 'PASS', coldFromBeforeWorkerMs: entry.coldFromBeforeWorkerMs,
        coldMessageMs: entry.cold.wallMs, warmMs: entry.warm.map((run) => run.wallMs),
        warmMedianMs: median(entry.warm.map((run) => run.wallMs)), metrics: entry.cold.result.metrics, output, budget });
    }
    assert.equal(browserReport.setup.length, cases.length);
    assert(browserReport.setup.every((entry) => entry.workerScope === 'DedicatedWorkerGlobalScope' &&
      entry.hasNativeImageData && entry.hasWebAssembly && Object.keys(entry.assetBytes).length === 5));
    return { chromeVersion, browserVersion: browserReport.browserVersion, userAgent: browserReport.userAgent,
      setup: browserReport.setup, requests, results, stderr: stderr.trim() };
  } finally {
    chrome.kill();
    await Promise.all([
      chromeClosed,
      new Promise((resolve) => server.close(resolve)),
    ]);
  }
}

export async function collectPublishedWasmEvidence({ version, runtime, chromePath }) {
  assert(/^\d+\.\d+\.\d+$/.test(version), 'explicit --version is required');
  assert(['node', 'browser', 'all'].includes(runtime), `required runtime ${runtime} is not implemented`);
  await fs.mkdir(outputDir, { recursive: true });
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-wasm-evidence-'));
  const sourceSha = command('git', ['rev-parse', 'HEAD'], root);
  const sourceFiles = ['test/benchmarks/wasm-upload-comparison.bench.js',
    'test/benchmarks/wasm-published-evidence.mjs', 'test/benchmarks/wasm-browser-worker.mjs',
    'test/benchmarks/wasm-browser-main.mjs'];
  const sourceHash = createHash('sha256');
  for (const file of sourceFiles) {
    sourceHash.update(file);
    sourceHash.update(await fs.readFile(path.join(root, file)));
  }
  const sourceFilesSha256 = sourceHash.digest('hex');
  const sourceDirty = Boolean(command('git', ['status', '--porcelain', '--', ...sourceFiles], root));
  const context = { generatedAt: new Date().toISOString(), sourceSha, publishedVersion: version,
    sourceDirty, sourceFilesSha256,
    command: `node test/benchmarks/wasm-upload-comparison.bench.js --runtime ${runtime} --version ${version}`,
    node: process.version, npm: command('npm', ['--version'], root), os: process.platform,
    arch: process.arch, registry, packages: null, importPath: null,
    isolatedInstall: directory, shim: 'Node ImageData class only; browser uses native ImageData',
    runtimeClassification: 'Node process or Chrome DedicatedWorkerGlobalScope; metrics.runtime is not runtime proof',
    fixtures: null,
    nodeResults: null, nodeTotals: null, browserResults: null, verdict: 'FAIL' };
  try {
    const packageInfo = await installPublished(version, directory);
    context.packages = packageInfo.packages;
    context.importPath = packageInfo.browserImport;
    const cases = await prepareCases(directory);
    context.fixtures = cases.map(({ id, input, inputBytes, inputSha256, inputMetadata, options, metadataCase }) =>
      ({ id, input: metadataCase ? 'artifacts/benchmark/wasm-metadata-input.jpg' : path.relative(root, input),
        generatedFrom: metadataCase ? 'test/benchmarks/corpus/images/metadata-1.jpg + keepMetadata() + withXmp()' : null,
        inputBytes, inputSha256, inputMetadata, options }));
    if (runtime === 'node' || runtime === 'all') {
      context.nodeResults = await runNode(packageInfo, directory, cases);
      context.nodeTotals = summarize(context.nodeResults);
    }
    if (runtime === 'browser' || runtime === 'all') {
      const bundle = await bundleBrowser(directory, packageInfo.packageDir);
      const browser = await runBrowser(directory, cases, bundle, chromePath);
      const used = ['/main.js', '/worker.js', ...Object.keys(bundle.assets)];
      const deploymentRawBytes = used.reduce((sum, name) => sum + bundle.delivery[name].rawBytes, 0);
      const deploymentGzipBytes = used.reduce((sum, name) => sum + bundle.delivery[name].gzipBytes, 0);
      const totalRunTransferredBodyBytes = browser.requests.filter((request) => used.includes(request.url))
        .reduce((sum, request) => sum + request.transferredBodyBytes, 0);
      const firstInputAfter = browser.requests.findIndex((request) => request.url === '/input/png-jpeg');
      const firstCaseTransferredBodyBytes = browser.requests.slice(0, firstInputAfter)
        .filter((request) => used.includes(request.url))
        .reduce((sum, request) => sum + request.transferredBodyBytes, 0);
      context.browserResults = { ...browser, totals: summarize(browser.results),
        delivery: bundle.delivery, packageDirectoryBytes: bundle.packageDirectoryBytes,
        bundlePackageInputs: bundle.bundlePackageInputs,
        requiredAssets: used, additionalChunks: [], deploymentRawBytes, deploymentGzipBytes,
        firstCaseTransferredBodyBytes, totalRunTransferredBodyBytes,
        cache: 'fresh Chrome profile, HTTP Cache-Control: no-store; each case uses a new Worker, warm samples reuse it',
        bundleTool: 'esbuild 0.25.10', loadMode: 'explicit wasmModules injection from Worker HTTP fetch' };
    }
    context.verdict = 'PASS';
  } catch (error) {
    context.error = { message: error.message, stack: error.stack };
  }
  const reportPath = path.join(outputDir, 'wasm-published-evidence.json');
  try {
    await fs.writeFile(reportPath, `${JSON.stringify(context, null, 2)}\n`);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
  if (context.verdict !== 'PASS') throw new Error(`Published Wasm evidence FAIL: ${context.error.message}; see ${reportPath}`);
  return context;
}
