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
export function assertMetadataInput(metadata) {
  const missing = ['exif', 'gpsTag', 'xmp', 'icc'].filter((key) => !metadata[key]);
  assert.equal(missing.length, 0, `metadata fixture missing required input: ${missing.join(', ')}`);
}
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

async function packageLicenseHashes(directory) {
  const hashes = {};
  async function visit(current, depth) {
    if (depth > 5) return;
    for (const entry of await fs.readdir(current, { withFileTypes: true })) {
      const file = path.join(current, entry.name);
      if (entry.isDirectory()) await visit(file, depth + 1);
      else if (entry.isFile() && /^LICEN[CS]E(?:\.|$)/i.test(entry.name)) {
        hashes[path.relative(directory, file)] = sha256(await fs.readFile(file));
      }
    }
  }
  await visit(directory, 0);
  return hashes;
}

async function pinnedLicense(url, expectedHash) {
  const response = await fetch(url, { signal: AbortSignal.timeout(10000) });
  assert(response.ok, `version-pinned license unavailable: ${url} HTTP ${response.status}`);
  const hash = sha256(Buffer.from(await response.arrayBuffer()));
  if (expectedHash) assert.equal(hash, expectedHash, `version-pinned license changed: ${url}`);
  return { url, sha256: hash };
}

async function installPublished(version, directory, runtime, candidateTarball) {
  await fs.writeFile(path.join(directory, 'package.json'), JSON.stringify({ private: true, type: 'module' }));
  const requested = [candidateTarball ?? `${packageName}@${version}`, 'esbuild@0.25.10'];
  if (runtime === 'edge' || runtime === 'all') requested.push('workerd@1.20260924.1');
  command('npm', ['install', '--registry', registry, '--save-exact', ...requested], directory);
  const lock = JSON.parse(await fs.readFile(path.join(directory, 'package-lock.json'), 'utf8'));
  const names = [packageName, '@jsquash/jpeg', '@jsquash/png', '@jsquash/resize', '@jsquash/webp',
    'esbuild', ...(runtime === 'edge' || runtime === 'all' ? ['workerd'] : [])];
  const installedOptional = async (prefix) => {
    const candidates = Object.keys(lock.packages).filter((key) => key.startsWith(`node_modules/${prefix}`));
    const present = [];
    for (const key of candidates) {
      try { await fs.access(path.join(directory, key)); present.push(key.slice('node_modules/'.length)); }
      catch (error) { if (error.code !== 'ENOENT') throw error; }
    }
    assert.equal(present.length, 1, `expected one installed ${prefix} binary package`);
    return present[0];
  };
  const esbuildBinaryPackage = await installedOptional('@esbuild/');
  const workerdBinaryPackage = runtime === 'edge' || runtime === 'all'
    ? await installedOptional('@cloudflare/workerd-') : null;
  names.push(esbuildBinaryPackage);
  if (workerdBinaryPackage) names.push(workerdBinaryPackage);
  const packages = Object.fromEntries(await Promise.all(names.map(async (name) => {
    const entry = lock.packages[`node_modules/${name}`];
    const candidateEntry = name === packageName && candidateTarball;
    assert(entry?.integrity && (candidateEntry
      ? entry.resolved?.startsWith('file:') &&
        await fs.realpath(fileURLToPath(entry.resolved)) === candidateTarball
      : entry.resolved?.startsWith(registry)), `package provenance missing: ${name}`);
    const packageDirectory = path.join(directory, 'node_modules', name);
    const manifestBytes = await fs.readFile(path.join(packageDirectory, 'package.json'));
    const manifest = JSON.parse(manifestBytes);
    return [name, { version: entry.version, resolved: entry.resolved, integrity: entry.integrity,
      licenseDeclared: manifest.license ?? null, packageJsonSha256: sha256(manifestBytes),
      licenseFiles: await packageLicenseHashes(packageDirectory) }];
  })));
  assert.equal(packages[packageName].version, version);
  const binaryHash = async (packageName, executable) =>
    sha256(await fs.readFile(path.join(directory, 'node_modules', packageName, 'bin', executable)));
  const toolchainBinaries = { esbuild: { package: esbuildBinaryPackage,
    sha256: await binaryHash(esbuildBinaryPackage, 'esbuild') } };
  assert.equal(toolchainBinaries.esbuild.sha256, await binaryHash('esbuild', 'esbuild'));
  if (workerdBinaryPackage) {
    toolchainBinaries.workerd = { package: workerdBinaryPackage,
      sha256: await binaryHash(workerdBinaryPackage, 'workerd') };
    assert.equal(toolchainBinaries.workerd.sha256, await binaryHash('workerd', 'workerd'));
  }
  if (candidateTarball) {
    assert.equal(command('git', ['status', '--porcelain', '--', 'LICENSE'], root), '',
      'candidate source LICENSE differs from recorded commit');
  }
  const sourceLicense = candidateTarball
    ? { path: path.join(root, 'LICENSE'), sourceCommit: command('git', ['rev-parse', 'HEAD'], root),
      sha256: sha256(await fs.readFile(path.join(root, 'LICENSE'))) }
    : await pinnedLicense(`https://raw.githubusercontent.com/albert-einshutoin/lazy-image/v${version}/LICENSE`,
      version === '1.3.1' ? 'ff1b6da07c1a09446754bf5e0fe61a788fc6815c0ea0517d385df0b725b2b539' : null);
  const workerdLicense = workerdBinaryPackage ? await pinnedLicense(
    'https://raw.githubusercontent.com/cloudflare/workerd/v1.20260924.1/LICENSE',
    '0d542e0c8804e39aa7f37eb00da5a762149dc682d7829451287e11b938e94594') : null;
  const licenseFallbacks = {
    [packageName]: sourceLicense,
    [esbuildBinaryPackage]: { package: 'esbuild', file: 'LICENSE.md',
      sha256: packages.esbuild.licenseFiles['LICENSE.md'] },
    ...(workerdBinaryPackage ? { workerd: workerdLicense, [workerdBinaryPackage]: workerdLicense } : {}),
  };
  for (const [name, entry] of Object.entries(packages)) {
    if (Object.keys(entry.licenseFiles).length) continue;
    entry.licenseSource = licenseFallbacks[name];
    assert(entry.licenseSource?.sha256, `canonical license source missing: ${name}`);
  }
  const packageDir = path.join(directory, 'node_modules', packageName);
  const browserImport = await fs.realpath(path.join(packageDir, 'browser.js'));
  assert(browserImport.startsWith((await fs.realpath(directory)) + path.sep));
  return { packages, packageDir, browserImport, toolchainBinaries };
}

async function prepareCases(directory) {
  const metadataInput = path.join(directory, 'metadata-exif-gps-xmp-icc.jpg');
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
    if (item.metadataCase) assertMetadataInput(item.inputMetadata);
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

async function bundleBrowser(directory, packageDir, browserLoad) {
  const browserDir = path.join(directory, 'browser');
  await fs.mkdir(path.join(browserDir, 'assets'), { recursive: true });
  const { build } = await import(pathToFileURL(path.join(directory, 'node_modules/esbuild/lib/main.js')).href);
  const workerPath = path.join(browserDir, 'worker.js');
  const isolatedEntry = path.join(directory, 'worker-entry.mjs');
  await fs.copyFile(path.join(sourceDir, browserLoad === 'default'
    ? 'wasm-browser-worker-default.mjs' : 'wasm-browser-worker.mjs'), isolatedEntry);
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
  const assetSources = {};
  const copyAsset = async (codec, relative, url) => {
    assert(!assets[url], `duplicate browser Wasm URL: ${url}`);
    const source = path.join(directory, 'node_modules', codec, relative);
    const target = path.join(browserDir, url.slice(1));
    await fs.copyFile(source, target);
    assets[url] = target;
    assetSources[url] = { package: codec, path: relative, sha256: sha256(await fs.readFile(source)) };
  };
  if (browserLoad === 'default') {
    for (const codec of ['@jsquash/jpeg', '@jsquash/png', '@jsquash/resize', '@jsquash/webp']) {
      const packageRoot = path.join(directory, 'node_modules', codec);
      const visit = async (current) => {
        for (const entry of await fs.readdir(current, { withFileTypes: true })) {
          const file = path.join(current, entry.name);
          if (entry.isDirectory()) await visit(file);
          else if (entry.isFile() && entry.name.endsWith('.wasm')) {
            await copyAsset(codec, path.relative(packageRoot, file), `/${entry.name}`);
          }
        }
      };
      await visit(packageRoot);
    }
  } else {
    for (const [name, [codec, relative]] of Object.entries(codecFiles)) {
      await copyAsset(codec, relative, `/assets/${name}`);
    }
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
  for (const [url, source] of Object.entries(assetSources)) {
    assert.equal(delivery[url].sha256, source.sha256, `copied Wasm changed: ${url}`);
  }
  return { browserDir, assets, assetSources, delivery, packageDirectoryBytes,
    bundleInputs: bundleInputs.map((input) => path.resolve(root, input)),
    bundlePackageInputs: bundleInputs.filter((input) => input.includes('node_modules/@alberteinshutoin/lazy-image-wasm/'))
      .map((input) => path.resolve(root, input)) };
}

async function runBrowser(directory, cases, bundle, chromePath, { browserLoad, withholdWasm, corruptJpeg }) {
  const chromeVersion = command(chromePath, ['--version'], directory);
  if (withholdWasm) {
    assert(browserLoad === 'default' && /^[\w-]+\.wasm$/.test(withholdWasm) &&
      bundle.assets[`/${withholdWasm}`], `invalid --withhold-wasm: ${withholdWasm}`);
  }
  const expectedCode = withholdWasm === 'mozjpeg_dec.wasm' ? 'E131' :
    withholdWasm === 'squoosh_resize_bg.wasm' ? 'E503' :
      withholdWasm === 'webp_enc_simd.wasm' ? 'E300' : corruptJpeg ? 'E131' : null;
  assert(!withholdWasm || expectedCode, `unsupported diagnostic fixture: ${withholdWasm}`);
  const requests = [];
  let receiveReport;
  const reportPromise = new Promise((resolve) => { receiveReport = resolve; });
  const config = cases.map(({ id, options, budgetCases }) => ({ id, options, budgetCases: Boolean(budgetCases) }));
  const inputs = Object.fromEntries(cases.map((item) => [item.id, item.input]));
  const server = createServer(async (request, response) => {
    try {
      const absoluteUrl = new URL(request.url, `http://${request.headers.host}`);
      const url = absoluteUrl.pathname;
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
        if (!file || url === `/${withholdWasm}`) {
          requests.push({ url, absoluteUrl: absoluteUrl.href, status: 404,
            contentType: 'text/plain', reason: url === `/${withholdWasm}` ? 'withheld Wasm' : 'not found' });
          response.writeHead(404, { 'content-type': 'text/plain', 'cache-control': 'no-store' });
          response.end('missing');
          return;
        }
        body = await fs.readFile(file);
        type = url.endsWith('.wasm') ? 'application/wasm' : url.endsWith('.js') ? 'text/javascript' :
          url.endsWith('.png') || url === '/input/png-jpeg' ? 'image/png' : 'image/jpeg';
      }
      const gzip = /\bgzip\b/.test(request.headers['accept-encoding'] ?? '');
      const transmitted = gzip ? zlib.gzipSync(body) : body;
      response.writeHead(200, { 'content-type': type, 'cache-control': 'no-store',
        ...(gzip ? { 'content-encoding': 'gzip' } : {}), 'content-length': transmitted.length });
      response.end(transmitted);
      requests.push({ url, absoluteUrl: absoluteUrl.href, status: 200, contentType: type,
        contentEncoding: gzip ? 'gzip' : null, rawBytes: body.length,
        transferredBodyBytes: transmitted.length, sha256: sha256(body) });
    } catch (error) {
      requests.push({ url: request.url, status: 500, reason: error.message });
      response.writeHead(500); response.end(error.stack);
      receiveReport({ error: { message: error.message } });
    }
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
  let browserReport;
  try {
    let timeout;
    browserReport = await Promise.race([reportPromise, chromeFailure,
      new Promise((_, reject) => { timeout = setTimeout(() => reject(new Error(`Chrome report timeout: ${stderr}`)), 300000); })])
      .finally(() => clearTimeout(timeout));
    assert(!browserReport.error, browserReport.error?.stack ?? browserReport.error?.message);
    assert(browserReport.userAgent.includes('Chrome/'));
    if (expectedCode) {
      assert.equal(cases.length, 1, 'diagnostic check must use one fresh Worker');
      assert.equal(browserReport.setup.length, 1);
      assert.equal(browserReport.setup[0].workerScope, 'DedicatedWorkerGlobalScope');
      const observed = browserReport.cases.find((item) => item.id === cases[0].id);
      assert(observed, `missing browser diagnostic case: ${cases[0].id}`);
      assert.equal(observed.cold.ok, false, 'image processing unexpectedly succeeded');
      const error = observed.cold.error;
      assert.equal(error?.code, expectedCode);
      assert.equal(error?.category, 'CodecError');
      assert.equal(error?.recoverable, false);
      assert.equal(typeof error?.message, 'string');
      assert(error?.recoveryHint?.includes('DevTools Network'));
      assert(error?.recoveryHint?.includes('If delivery is valid'));
      const wasmUrl = withholdWasm ? `/${withholdWasm}` : '/mozjpeg_dec.wasm';
      assert(error.recoveryHint.includes(path.basename(wasmUrl)));
      const wasmRequests = requests.filter((request) => request.url === wasmUrl);
      assert(wasmRequests.length > 0, `codec did not request ${wasmUrl}`);
      assert(wasmRequests.every((request) => request.status === (withholdWasm ? 404 : 200)),
        `unexpected ${wasmUrl} HTTP result`);
      if (corruptJpeg) {
        assert(requests.filter((request) => request.url.endsWith('.wasm'))
          .every((request) => request.status === 200), 'corrupt input had a Wasm delivery failure');
        assert(!/\b(?:missing asset|HTTP 404|not found)\b/i.test(error.message),
          'corrupt input was diagnosed as missing Wasm');
      }
      return { chromeVersion, browserVersion: browserReport.browserVersion,
        userAgent: browserReport.userAgent, setup: browserReport.setup, requests,
        expectedFailure: { validationStatus: 'PASS', imageProcessingOutcome: 'FAIL',
          case: cases[0].id, expectedCode, workerError: error, wasmRequests,
          networkEvidenceSource: 'verification HTTP server, not the package',
          freshWorker: true }, stderr: stderr.trim() };
    }
    const results = [];
    for (const item of cases) {
      const entry = browserReport.cases.find((candidate) => candidate.id === item.id);
      assert(entry, `missing browser case: ${item.id}`);
      assert(entry.cold.ok, `${item.id}: ${entry.cold.error?.stack ?? entry.cold.error?.message}`);
      assert(entry.warm.every((run) => run.ok));
      const outputBytes = Buffer.from(entry.cold.result.data);
      const output = await validateOutput(outputBytes, entry.cold.result, item,
        path.join(outputDir, `wasm-browser-${browserLoad === 'default' ? 'default-' : ''}${item.id}.${item.options.format === 'jpeg' ? 'jpg' : 'webp'}`));
      let budget = null;
      if (item.budgetCases) {
        assert(entry.bestEffort.ok);
        assert.equal(entry.bestEffort.result.budgetMet, false);
        assert(entry.bestEffort.result.bytesOut > 10);
        assert.equal(entry.strict.error?.code, 'E502');
        assert.equal(entry.strict.error?.category, 'ResourceLimit');
        const bestEffortOutput = await validateOutput(Buffer.from(entry.bestEffort.result.data),
          entry.bestEffort.result, item, path.join(outputDir,
            `wasm-browser-${browserLoad === 'default' ? 'default-' : ''}${item.id}-best-effort.jpg`));
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
      entry.hasNativeImageData && entry.hasWebAssembly &&
      Object.keys(entry.assetBytes).length === (browserLoad === 'default' ? 0 : 5)));
    return { chromeVersion, browserVersion: browserReport.browserVersion, userAgent: browserReport.userAgent,
      setup: browserReport.setup, requests, results, stderr: stderr.trim() };
  } catch (error) {
    error.partialBrowserResults = { chromeVersion, requests,
      cases: browserReport?.cases?.map((entry) => ({ id: entry.id,
        cold: { ok: entry.cold.ok, error: entry.cold.error },
        warm: entry.warm.map((run) => ({ ok: run.ok, error: run.error })) })) ?? [],
      setup: browserReport?.setup ?? [], reportError: browserReport?.error ?? null, stderr: stderr.trim() };
    throw error;
  } finally {
    chrome.kill();
    await Promise.all([
      chromeClosed,
      new Promise((resolve) => server.close(resolve)),
    ]);
  }
}

export async function collectPublishedWasmEvidence({ version, runtime, chromePath, workerdPath,
  browserLoad = 'injected', withholdWasm = null, candidateTarball = null, corruptJpeg = false,
  edgeDiagnostic = null }) {
  assert(/^\d+\.\d+\.\d+$/.test(version), 'explicit --version is required');
  assert(['node', 'browser', 'edge', 'all'].includes(runtime), `required runtime ${runtime} is not implemented`);
  assert(['injected', 'default'].includes(browserLoad), `unsupported browser load mode: ${browserLoad}`);
  assert(!withholdWasm || (browserLoad === 'default' && runtime === 'browser'),
    '--withhold-wasm requires a default-load browser run');
  assert(!corruptJpeg || (browserLoad === 'default' && runtime === 'browser' && !withholdWasm),
    '--corrupt-jpeg requires a default-load browser run without withheld Wasm');
  assert(!edgeDiagnostic || (runtime === 'edge' &&
    ['corrupt-jpeg', 'decoder-init', 'resize-init', 'encoder-init'].includes(edgeDiagnostic)),
  '--edge-diagnostic requires edge and a supported diagnostic case');
  if (candidateTarball) {
    candidateTarball = path.resolve(candidateTarball);
    assert(!candidateTarball.startsWith(root + path.sep), 'candidate tarball must be outside checkout');
    candidateTarball = await fs.realpath(candidateTarball);
    assert(!candidateTarball.startsWith(root + path.sep), 'candidate tarball must be outside checkout');
  }
  await fs.mkdir(outputDir, { recursive: true });
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-wasm-evidence-'));
  const sourceSha = command('git', ['rev-parse', 'HEAD'], root);
  const sourceFiles = ['test/benchmarks/wasm-upload-comparison.bench.js',
    'test/benchmarks/wasm-published-evidence.mjs', 'test/benchmarks/wasm-browser-worker.mjs',
    'test/benchmarks/wasm-browser-main.mjs', 'test/benchmarks/wasm-browser-worker-default.mjs',
    'test/benchmarks/wasm-edge-workerd.mjs',
    'test/benchmarks/wasm-edge-worker.mjs'];
  const sourceHash = createHash('sha256');
  for (const file of sourceFiles) {
    sourceHash.update(file);
    sourceHash.update(await fs.readFile(path.join(root, file)));
  }
  const sourceFilesSha256 = sourceHash.digest('hex');
  const sourceDirty = Boolean(command('git', ['status', '--porcelain', '--', ...sourceFiles], root));
  const packageSource = candidateTarball ? { kind: 'candidate-tarball', path: candidateTarball,
    sha256: sha256(await fs.readFile(candidateTarball)), sourceCommit: sourceSha,
    sourceDirty: Boolean(command('git', ['status', '--porcelain', '--', 'packages/lazy-image-wasm'], root)) } :
    { kind: 'published-registry', registry };
  const context = { generatedAt: new Date().toISOString(), sourceSha,
    ...(candidateTarball ? { candidateVersion: version } : { publishedVersion: version }),
    sourceDirty, sourceFilesSha256, packageSource,
    command: `node test/benchmarks/wasm-upload-comparison.bench.js --runtime ${runtime} --version ${version}${['browser', 'all'].includes(runtime) ? ` --browser-load ${browserLoad}` : ''}${candidateTarball ? ` --candidate-tarball ${candidateTarball}` : ''}${withholdWasm ? ` --withhold-wasm ${withholdWasm}` : ''}${corruptJpeg ? ' --corrupt-jpeg' : ''}${edgeDiagnostic ? ` --edge-diagnostic ${edgeDiagnostic}` : ''}${workerdPath ? ` --workerd ${workerdPath}` : ''}`,
    node: process.version, npm: null, os: process.platform,
    osVersion: process.platform === 'darwin' ? command('/usr/bin/sw_vers', ['-productVersion'], root) : os.version(),
    osRelease: os.release(), arch: process.arch, registry, packages: null, toolchainBinaries: null, importPath: null,
    isolatedInstall: directory, shim: 'Node ImageData class only; browser uses native ImageData; Edge adapter adds no ImageData or DOM shim',
    runtimeClassification: 'Node process, Chrome DedicatedWorkerGlobalScope, or local workerd isolate; metrics.runtime is not runtime proof',
    fixtures: null,
    nodeResults: null, nodeTotals: null, browserLoadMode: ['browser', 'all'].includes(runtime) ? browserLoad : null,
    browserResults: null, edgeResults: null, edgeTotals: null,
    diagnosticValidation: withholdWasm || corruptJpeg || edgeDiagnostic ? { status: 'NOT_RUN' } : null,
    verdict: 'FAIL' };
  try {
    // Both browser and Edge bundling must use the hashed esbuild from this install.
    if (process.env.ESBUILD_BINARY_PATH !== undefined) {
      throw new Error('external esbuild override is unsupported: unset ESBUILD_BINARY_PATH');
    }
    context.npm = command('npm', ['--version'], root);
    const packageInfo = await installPublished(version, directory, runtime, candidateTarball);
    if (candidateTarball) {
      for (const file of ['package.json', 'browser.js', 'worker.js', 'shared.js', 'edge.js', 'README.md']) {
        assert.equal(sha256(await fs.readFile(path.join(packageInfo.packageDir, file))),
          sha256(await fs.readFile(path.join(root, 'packages/lazy-image-wasm', file))),
          `candidate tarball differs from checkout: ${file}`);
      }
    }
    context.packages = packageInfo.packages;
    context.toolchainBinaries = packageInfo.toolchainBinaries;
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
      let browserCases = browserLoad === 'default' ? [{
        id: 'probe-jpeg-webp', input: path.join(root, 'test/fixtures/test_100KB_1057x1057.jpg'),
        options: { format: 'webp', maxWidth: 320, maxHeight: 320, targetBytes: 50000,
          minQuality: 45, maxQuality: 86, qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' },
      }, ...cases] : cases;
      if (corruptJpeg) {
        const corruptInput = path.join(directory, 'corrupt-jpeg.jpg');
        await fs.writeFile(corruptInput, Buffer.from([0xff, 0xd8, 0xff, 0, 0, 0]));
        browserCases = [{ id: 'corrupt-jpeg', input: corruptInput,
          options: { format: 'webp', output: 'arrayBuffer' } }];
      }
      if (withholdWasm) browserCases = [browserCases[0]];
      if (browserLoad === 'default') {
        const probeBytes = await fs.readFile(browserCases[0].input);
        context.browserProbeInput = { path: corruptJpeg ? 'generated malformed JPEG header' :
          path.relative(root, browserCases[0].input),
          sha256: sha256(probeBytes), options: browserCases[0].options };
      }
      const bundle = await bundleBrowser(directory, packageInfo.packageDir, browserLoad);
      const browser = await runBrowser(directory, browserCases, bundle, chromePath,
        { browserLoad, withholdWasm, corruptJpeg });
      if (browser.expectedFailure) {
        context.browserResults = { ...browser, bundlePackageInputs: bundle.bundlePackageInputs,
          copiedWasmSources: bundle.assetSources, copiedAssets: Object.keys(bundle.assets),
          loadMode: `${candidateTarball ? 'candidate' : 'published'} Worker helper and codec default loader; no wasmModules` };
        context.diagnosticValidation = { status: 'PASS', scenario: browser.expectedFailure.case,
          expectedCode: browser.expectedFailure.expectedCode };
      } else {
      const used = [...new Set(browser.requests.filter((request) => request.status === 200 &&
        (request.url.endsWith('.js') || request.url.endsWith('.wasm'))).map((request) => request.url))];
      assert(used.includes('/main.js') && used.includes('/worker.js'));
      const requestedWasm = used.filter((url) => url.endsWith('.wasm')).map((url) => ({
        url, source: bundle.assetSources[url], delivery: bundle.delivery[url] }));
      assert(requestedWasm.length > 0 && requestedWasm.every((asset) => asset.source && asset.delivery),
        'published codec Wasm requests did not match copied package assets');
      const results = browser.results.filter((entry) => entry.id !== 'probe-jpeg-webp');
      const probe = browser.results.find((entry) => entry.id === 'probe-jpeg-webp') ?? null;
      const deploymentRawBytes = used.reduce((sum, name) => sum + bundle.delivery[name].rawBytes, 0);
      const deploymentGzipBytes = used.reduce((sum, name) => sum + bundle.delivery[name].gzipBytes, 0);
      const totalRunTransferredBodyBytes = browser.requests.filter((request) => request.status === 200 &&
        used.includes(request.url))
        .reduce((sum, request) => sum + request.transferredBodyBytes, 0);
      const firstInputAfter = browser.requests.findIndex((request) =>
        request.url === (browserLoad === 'default' ? '/input/jpeg-webp' : '/input/png-jpeg'));
      assert(firstInputAfter > 0, 'first-case transfer boundary unavailable');
      const firstCaseTransferredBodyBytes = browser.requests.slice(0, firstInputAfter)
        .filter((request) => request.status === 200 && used.includes(request.url))
        .reduce((sum, request) => sum + request.transferredBodyBytes, 0);
      context.browserResults = { ...browser, results, probe, totals: summarize(results),
        delivery: bundle.delivery, packageDirectoryBytes: bundle.packageDirectoryBytes,
        bundlePackageInputs: bundle.bundlePackageInputs, bundleInputs: bundle.bundleInputs,
        copiedWasmSources: bundle.assetSources, requestedWasm, copiedAssets: Object.keys(bundle.assets),
        requiredAssets: used, additionalChunks: [], deploymentRawBytes, deploymentGzipBytes,
        firstCaseTransferredBodyBytes, totalRunTransferredBodyBytes,
        cache: 'fresh Chrome profile, HTTP Cache-Control: no-store; each case uses a new Worker, warm samples reuse it',
        bundleTool: 'esbuild 0.25.10', loadMode: browserLoad === 'default'
          ? `${candidateTarball ? 'candidate' : 'published'} Worker helper and codec default loader; no wasmModules`
          : 'explicit wasmModules injection from Worker HTTP fetch' };
      }
    }
    if (runtime === 'edge' || runtime === 'all') {
      const { runEdgeWorkerd } = await import('./wasm-edge-workerd.mjs');
      context.edgeResults = await runEdgeWorkerd({ directory, packageInfo, cases, codecFiles,
        outputDir, validateOutput, workerdPath, diagnosticCase: edgeDiagnostic });
      if (edgeDiagnostic) context.diagnosticValidation = context.edgeResults.diagnosticValidation;
      else context.edgeTotals = summarize(context.edgeResults.results);
    }
    context.verdict = context.browserResults?.expectedFailure || edgeDiagnostic ? 'FAIL' : 'PASS';
    if (context.browserResults?.expectedFailure || edgeDiagnostic) {
      context.error = { message: `Intentional image-processing FAIL; diagnostic ${context.diagnosticValidation?.status ?? 'NOT_RUN'}`, stack: null };
    }
  } catch (error) {
    if (error.partialEdgeResults) context.edgeResults = error.partialEdgeResults;
    if (error.partialBrowserResults) context.browserResults = error.partialBrowserResults;
    context.verdict = error.verdict ?? 'FAIL';
    if (context.diagnosticValidation) context.diagnosticValidation = { status: 'FAIL', reason: error.message };
    context.error = { message: error.message, stack: error.stack };
  }
  const reportPath = path.join(outputDir, 'wasm-published-evidence.json');
  try {
    await fs.writeFile(reportPath, `${JSON.stringify(context, null, 2)}\n`);
  } finally {
    await fs.rm(directory, { recursive: true, force: true });
  }
  if (context.verdict !== 'PASS') throw new Error(`${candidateTarball ? 'Candidate' : 'Published'} Wasm evidence ${context.verdict}: ${context.error.message}; see ${reportPath}`);
  return context;
}
