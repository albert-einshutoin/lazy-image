import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createServer } from 'node:http';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import zlib from 'node:zlib';
import sharp from 'sharp';
import { sha256, readChecked } from './wasm-quality-metrics.mjs';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const target = path.join(root, 'docs/history/v1.4.0/quality-evaluation');
const registry = 'https://registry.npmjs.org/';
const pkg = '@alberteinshutoin/lazy-image-wasm';
const expectedIntegrity = 'sha512-R3xpMJr3rBv+LsyfpggixCZvktlS3XRvLi542x4JgbrvvBP3NH/xip3TJSze4fGDQIfj7hWplLeKUlHPCnidew==';
const chromePath = process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const sourceFiles = ['test/benchmarks/wasm-quality-photo-run.mjs',
  'test/benchmarks/wasm-quality-browser-main.mjs',
  'test/benchmarks/wasm-browser-worker-default.mjs'];

function command(file, args, cwd) {
  const run = spawnSync(file, args, { cwd, encoding: 'utf8', maxBuffer: 8 * 1024 * 1024 });
  if (run.error || run.status !== 0) throw new Error(`${file} ${args.join(' ')}: ${run.error?.message || run.stderr}`);
  return run.stdout.trim();
}

async function run() {
  if (process.env.ESBUILD_BINARY_PATH) throw new Error('external esbuild override unsupported');
  const manifest = JSON.parse(await fs.readFile(path.join(root, 'test/benchmarks/corpus/release-manifest.json')));
  const inputs = Object.fromEntries(await Promise.all(['coffee', 'chelsea'].map(async (id) => {
    const entry = manifest.entries.find((item) => item.id === id);
    assert(entry && entry.license === 'CC0-1.0' && entry.category === 'photo');
    const file = path.join(root, entry.path);
    const bytes = await readChecked(file, entry.sha256);
    assert.equal(bytes.length, entry.bytes);
    return [id, { file, entry, bytes }];
  })));
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-wasm-quality-'));
  const codeSha = command('git', ['rev-parse', 'HEAD'], root);
  const codeDirty = Boolean(command('git', ['status', '--porcelain', '--', ...sourceFiles], root));
  assert(!codeDirty, 'measurement code must be committed before image generation');
  const sourceHashes = Object.fromEntries(await Promise.all(sourceFiles.map(async (name) =>
    [name, sha256(await fs.readFile(path.join(root, name)))])));
  const evidence = { generatedAt: new Date().toISOString(), imageGenerationCodeSha: codeSha,
    imageGenerationSourceHashes: sourceHashes, sourceDirty: false,
    command: 'node test/benchmarks/wasm-quality-photo-run.mjs',
    environment: { platform: process.platform, arch: process.arch, node: process.version,
      npm: command('npm', ['--version'], root),
      osVersion: process.platform === 'darwin' ? command('/usr/bin/sw_vers', ['-productVersion'], root) : os.release(),
      chrome: command(chromePath, ['--version'], root),
      runtime: 'Chrome DedicatedWorkerGlobalScope, same-origin HTTP, Cache-Control: no-store',
      worker: 'published /worker; no wasmModules', bundler: 'esbuild 0.25.10' },
    package: null, inputs: null, assets: null, requests: [], cases: [], verdict: 'FAIL' };
  let server;
  let chrome;
  try {
    await fs.writeFile(path.join(directory, 'package.json'), JSON.stringify({ private: true, type: 'module' }));
    command('npm', ['install', '--registry', registry, '--save-exact', `${pkg}@1.4.0`, 'esbuild@0.25.10'], directory);
    const lock = JSON.parse(await fs.readFile(path.join(directory, 'package-lock.json'), 'utf8'));
    const packageEntry = lock.packages[`node_modules/${pkg}`];
    assert.equal(packageEntry.version, '1.4.0');
    assert.equal(packageEntry.integrity, expectedIntegrity);
    const registryIntegrity = command('npm', ['view', `${pkg}@1.4.0`, 'dist.integrity', '--registry', registry], directory);
    assert.equal(registryIntegrity, expectedIntegrity);
    const packageDir = path.join(directory, 'node_modules', pkg);
    const packageManifest = JSON.parse(await fs.readFile(path.join(packageDir, 'package.json')));
    assert.equal(packageManifest.exports['./worker'].import, './worker.js');
    const { VERSION } = await import(pathToFileURL(path.join(packageDir, 'shared.js')).href);
    assert.equal(VERSION, '1.4.0');
    const installedPackages = Object.fromEntries([pkg, '@jsquash/jpeg', '@jsquash/png', '@jsquash/resize',
      '@jsquash/webp', 'esbuild'].map((name) => [name, lock.packages[`node_modules/${name}`]]));
    assert(Object.values(installedPackages).every((item) => item?.integrity));
    evidence.package = { registry, tarballIntegrity: expectedIntegrity, registryIntegrity,
      packageJsonSha256: sha256(await fs.readFile(path.join(packageDir, 'package.json'))),
      workerSha256: sha256(await fs.readFile(path.join(packageDir, 'worker.js'))),
      browserSha256: sha256(await fs.readFile(path.join(packageDir, 'browser.js'))),
      sharedSha256: sha256(await fs.readFile(path.join(packageDir, 'shared.js'))),
      installedPackages: Object.fromEntries(Object.entries(installedPackages).map(([name, item]) =>
        [name, { version: item.version, resolved: item.resolved, integrity: item.integrity }])) };
    evidence.inputs = Object.fromEntries(Object.entries(inputs).map(([id, item]) => [id,
      { path: item.entry.path, bytes: item.bytes.length, sha256: item.entry.sha256,
        category: 'photo', license: item.entry.license, source: item.entry.source }]));

    const entryPath = path.join(directory, 'worker-entry.mjs');
    await fs.copyFile(path.join(root, 'test/benchmarks/wasm-browser-worker-default.mjs'), entryPath);
    const workerPath = path.join(directory, 'worker.js');
    const { build } = await import(pathToFileURL(path.join(directory, 'node_modules/esbuild/lib/main.js')).href);
    const built = await build({ entryPoints: [entryPath], outfile: workerPath,
      bundle: true, format: 'esm', platform: 'browser', target: 'es2022', metafile: true });
    assert(Object.keys(built.metafile.inputs).some((name) => name.includes('node_modules/@alberteinshutoin/lazy-image-wasm/worker.js')));
    const isolatedRoot = await fs.realpath(directory);
    assert(Object.keys(built.metafile.inputs).every((name) =>
      path.resolve(root, name).startsWith(isolatedRoot + path.sep)));
    const assets = { '/worker.js': workerPath,
      '/main.js': path.join(root, 'test/benchmarks/wasm-quality-browser-main.mjs') };
    const wasmSources = {};
    for (const codec of ['@jsquash/jpeg', '@jsquash/png', '@jsquash/resize', '@jsquash/webp']) {
      const base = path.join(directory, 'node_modules', codec);
      async function visit(dir) {
        for (const item of await fs.readdir(dir, { withFileTypes: true })) {
          const file = path.join(dir, item.name);
          if (item.isDirectory()) await visit(file);
          else if (item.isFile() && item.name.endsWith('.wasm')) {
            assert(!assets[`/${item.name}`], `duplicate Wasm filename ${item.name}`);
            assets[`/${item.name}`] = file;
            wasmSources[`/${item.name}`] = { package: codec,
              relativePath: path.relative(base, file), sha256: sha256(await fs.readFile(file)) };
          }
        }
      }
      await visit(base);
    }
    evidence.assets = { workerBundleSha256: sha256(await fs.readFile(workerPath)),
      bundleInputs: Object.keys(built.metafile.inputs).map((name) => path.resolve(root, name)),
      copiedWasmSources: wasmSources };

    let receiveReport;
    const reportPromise = new Promise((resolve) => { receiveReport = resolve; });
    server = createServer(async (request, response) => {
      try {
        const url = new URL(request.url, `http://${request.headers.host}`).pathname;
        if (url === '/report' && request.method === 'POST') {
          const chunks = [];
          for await (const chunk of request) chunks.push(chunk);
          receiveReport(JSON.parse(Buffer.concat(chunks).toString('utf8')));
          response.writeHead(200); response.end('ok'); return;
        }
        const inputId = url.startsWith('/input/') ? url.slice('/input/'.length) : null;
        const body = url === '/' ? Buffer.from('<!doctype html><pre id="status">Running</pre><script type="module" src="/main.js"></script>')
          : inputId && inputs[inputId] ? inputs[inputId].bytes
            : assets[url] ? await fs.readFile(assets[url]) : null;
        if (!body) {
          evidence.requests.push({ url, status: 404 });
          response.writeHead(404, { 'cache-control': 'no-store' }); response.end('missing'); return;
        }
        const type = url.endsWith('.wasm') ? 'application/wasm' : url.endsWith('.js') ? 'text/javascript'
          : inputId ? 'image/png' : 'text/html';
        const gzip = /\bgzip\b/.test(request.headers['accept-encoding'] || '');
        const sent = gzip ? zlib.gzipSync(body) : body;
        response.writeHead(200, { 'content-type': type, 'cache-control': 'no-store',
          'content-length': sent.length, ...(gzip ? { 'content-encoding': 'gzip' } : {}) });
        response.end(sent);
        evidence.requests.push({ url, status: 200, contentType: type, contentEncoding: gzip ? 'gzip' : null,
          bytes: body.length, sha256: sha256(body) });
      } catch (error) { response.writeHead(500); response.end(error.message); }
    });
    await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve));
    const port = server.address().port;
    const profile = path.join(directory, 'chrome-profile');
    chrome = spawn(chromePath, ['--headless=new', '--no-first-run', '--disable-background-networking',
      `--user-data-dir=${profile}`, `http://127.0.0.1:${port}/`], { stdio: ['ignore', 'ignore', 'pipe'] });
    let stderr = '';
    chrome.stderr.on('data', (chunk) => { stderr = (stderr + chunk.toString()).slice(-10000); });
    let timer;
    const browser = await Promise.race([reportPromise,
      new Promise((_, reject) => chrome.once('error', reject)),
      new Promise((_, reject) => chrome.once('exit', (code) => reject(new Error(`Chrome exit ${code}: ${stderr}`)))),
      new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`Chrome timeout: ${stderr}`)), 300000); }),
    ]).finally(() => clearTimeout(timer));
    assert(!browser.error, browser.error?.stack || browser.error?.message);
    assert.equal(browser.cases.length, 12);
    assert.equal(browser.setup.length, 4);
    assert(browser.setup.every((item) => item.workerScope === 'DedicatedWorkerGlobalScope' &&
      item.hasNativeImageData && item.hasWebAssembly && Object.keys(item.assetBytes).length === 0));
    evidence.environment.browserVersion = browser.browserVersion;
    evidence.environment.userAgent = browser.userAgent;
    evidence.setup = browser.setup;
    await fs.mkdir(target, { recursive: true });
    for (const item of browser.cases) {
      const { output, ...row } = item;
      if (item.ok) {
        const bytes = Buffer.from(output);
        const metadata = await sharp(bytes).metadata();
        await sharp(bytes).raw().toBuffer();
        assert.equal(metadata.format, item.format);
        assert.equal(metadata.width, item.result.metrics.widthOut);
        assert.equal(metadata.height, item.result.metrics.heightOut);
        assert.equal(bytes.length, item.result.bytesOut);
        assert.equal(item.result.budgetMet, item.targetBytes === null || bytes.length <= item.targetBytes);
        assert.equal(item.result.metrics.budgetMet, item.result.budgetMet);
        const filename = `${item.inputId}-${item.format}-${item.condition === 'none' ? 'none' : item.condition === '80%' ? '80' : '50'}.${item.format === 'jpeg' ? 'jpg' : 'webp'}`;
        await fs.writeFile(path.join(target, filename), bytes);
        row.output = { file: filename, sha256: sha256(bytes), bytes: bytes.length,
          width: metadata.width, height: metadata.height, format: metadata.format };
      } else row.output = null;
      evidence.cases.push(row);
    }
    const wasmRequests = evidence.requests.filter((item) => item.url.endsWith('.wasm'));
    assert(wasmRequests.length > 0 && wasmRequests.every((item) => item.status === 200));
    evidence.requestedWasm = [...new Set(wasmRequests.map((item) => item.url))].map((url) =>
      ({ url, ...wasmSources[url] }));
    assert(evidence.requestedWasm.every((item) => item.sha256));
    evidence.verdict = 'PASS';
  } finally {
    chrome?.kill();
    if (server) await new Promise((resolve) => server.close(resolve));
    await fs.rm(directory, { recursive: true, force: true });
    await fs.mkdir(target, { recursive: true });
    await fs.writeFile(path.join(target, 'photo-run-evidence.json'), `${JSON.stringify(evidence, null, 2)}\n`);
  }
  assert.equal(evidence.verdict, 'PASS');
  console.log(`Saved ${evidence.cases.length} cases; ${evidence.requestedWasm.map((item) => item.url).join(', ')}`);
}

run().catch((error) => { console.error(error.stack || error.message); process.exitCode = 1; });
