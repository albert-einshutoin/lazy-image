'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const zlib = require('node:zlib');
const crypto = require('node:crypto');
const { createRequire } = require('node:module');
const digest = value => crypto.createHash('sha256').update(value).digest('hex');
const median = values => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
const summary = values => ({ median: median(values), min: Math.min(...values), max: Math.max(...values), values });
const readJson = file => JSON.parse(fs.readFileSync(file));
function setup(root) {
  const fromRoot = createRequire(path.join(root, 'package.json'));
  const sharp = fromRoot('sharp');
  const bindingName = process.platform === 'darwin' && process.arch === 'arm64'
    ? 'lazy-image.darwin-arm64.node'
    : process.platform === 'linux' && process.arch === 'x64'
      ? 'lazy-image.linux-x64-gnu.node'
      : null;
  assert(bindingName, `unsupported comparison host: ${process.platform}/${process.arch}`);
  const binding = path.join(root, bindingName);
  process.env.NAPI_RS_NATIVE_LIBRARY_PATH = binding;
  const { ImageEngine } = fromRoot('./index.js');
  assert(require.cache[binding], 'local binding was not loaded');
  assert.equal(ImageEngine, fromRoot(binding).ImageEngine);
  return { fromRoot, sharp, binding, ImageEngine };
}
function opaqueRgbaPng(root) {
  const { pngChunk } = createRequire(path.join(root, 'package.json'))('./test/helpers/png-helpers.js');
  const width = 1200, height = 900;
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(width, 0); ihdr.writeUInt32BE(height, 4);
  ihdr[8] = 8; ihdr[9] = 6;
  const stride = width * 4 + 1;
  const rows = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const p = y * stride + 1 + x * 4;
      rows[p] = (x * 17 + y * 3) & 255;
      rows[p + 1] = (x * 5 + y * 7) & 255;
      rows[p + 2] = (x ^ y) & 255;
      rows[p + 3] = 255;
    }
  }
  return Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    pngChunk('IHDR', ihdr), pngChunk('IDAT', zlib.deflateSync(rows)),
    pngChunk('IEND', Buffer.alloc(0)),
  ]);
}
async function normalized(sharp, input) {
  const { data, info } = await sharp(input).toColourspace('srgb').ensureAlpha().raw().toBuffer({ resolveWithObject: true });
  assert.equal(info.channels, 4);
  assert.equal(info.depth, 'uchar');
  const alpha = { transparent: 0, semi: 0, opaque: 0, hiddenRgb: 0 };
  for (let p = 0; p < data.length; p += 4) {
    if (data[p + 3] === 0) {
      alpha.transparent++;
      if (data[p] || data[p + 1] || data[p + 2]) alpha.hiddenRgb++;
    } else if (data[p + 3] === 255) alpha.opaque++;
    else alpha.semi++;
  }
  return { width: info.width, height: info.height, sha256: digest(data), alpha, data };
}
async function run() {
  const root = path.resolve(process.env.RESIZE_ROOT || '');
  const outDir = path.resolve(process.env.RESIZE_OUT_DIR || '');
  const revision = process.env.RESIZE_REVISION;
  const label = process.env.RESIZE_LABEL;
  const warmCount = Number(process.env.RESIZE_WARM_COUNT || 5);
  assert(root && outDir && revision && label, 'RESIZE_ROOT, RESIZE_OUT_DIR, RESIZE_REVISION, RESIZE_LABEL required');
  assert(Number.isInteger(warmCount) && warmCount >= 1 && warmCount <= 50, 'RESIZE_WARM_COUNT must be 1..50');
  const { sharp, binding, ImageEngine } = setup(root);
  fs.mkdirSync(outDir, { recursive: true });
  const fixture = file => fs.readFileSync(path.join(root, 'test/benchmarks/corpus/images', file));
  const cases = [
    { id: 'photo_inside', input: fixture('chelsea.png'), resize: { width: 237, height: 157, fit: 'inside' } },
    { id: 'ui_odd_fill', input: fixture('ui-1.png'), resize: { width: 157, height: 91, fit: 'fill' } },
    { id: 'alpha_cover', input: fixture('alpha-1.png'), resize: { width: 137, height: 95, fit: 'cover' } },
    { id: 'opaque_rgba_1mp_fill', input: opaqueRgbaPng(root), resize: { width: 511, height: 341, fit: 'fill' } },
  ];
  const results = [];
  for (const c of cases) {
    const meta = await sharp(c.input).metadata();
    if (c.id === 'opaque_rgba_1mp_fill') {
      assert.equal(meta.channels, 4);
      assert.equal(meta.width * meta.height, 1_080_000);
    }
    const invoke = () => ImageEngine.from(c.input).resize(c.resize).toBufferWithMetrics('png');
    const t0 = process.hrtime.bigint();
    const cold = await invoke();
    const coldWallMs = Number(process.hrtime.bigint() - t0) / 1e6;
    const warm = [];
    for (let i = 0; i < warmCount; i++) {
      const start = process.hrtime.bigint();
      const result = await invoke();
      warm.push({ wallMs: Number(process.hrtime.bigint() - start) / 1e6, metrics: result.metrics });
    }
    const output = cold.data;
    assert(Buffer.isBuffer(output), 'expected Buffer output');
    const pixels = await normalized(sharp, output);
    fs.writeFileSync(path.join(outDir, `${c.id}.png`), output);
    results.push({
      id: c.id,
      resize: c.resize,
      input: { bytes: c.input.length, sha256: digest(c.input), width: meta.width, height: meta.height, channels: meta.channels },
      output: { bytes: output.length, sha256: digest(output), width: pixels.width, height: pixels.height, pixelSha256: pixels.sha256, alpha: pixels.alpha },
      cold: { wallMs: coldWallMs, metrics: cold.metrics },
      warm: {
        wallMs: summary(warm.map(v => v.wallMs)),
        opsMs: summary(warm.map(v => v.metrics.opsMs)),
        encodeMs: summary(warm.map(v => v.metrics.encodeMs)),
        totalMs: summary(warm.map(v => v.metrics.totalMs)),
      },
    });
    console.log(`${label} ${c.id} ${pixels.width}x${pixels.height} ${pixels.sha256} wall ${results.at(-1).warm.wallMs.median.toFixed(2)}ms ops ${results.at(-1).warm.opsMs.median.toFixed(2)}ms`);
  }
  const runtime = { arch: process.arch, platform: process.platform, node: process.version,
    sharp: require(path.join(root, 'node_modules/sharp/package.json')).version,
    binding: path.basename(binding), bindingSha256: digest(fs.readFileSync(binding)),
    rayonThreads: process.env.RAYON_NUM_THREADS || null,
    rust: process.env.RUSTUP_TOOLCHAIN || null,
    sdk: process.env.SDKROOT || null,
    timing: `one cold and ${warmCount} sequential warm calls, ImageEngine.from through awaited toBufferWithMetrics(png); opsMs/encodeMs/totalMs are native metrics`,
    normalization: 'sharp toColourspace(srgb) ensureAlpha raw uchar RGBA' };
  const report = { label, revision, runtime, cases: results };
  fs.writeFileSync(path.join(outDir, 'results.json'), JSON.stringify(report, null, 2) + '\n');
}
async function compare() {
  const [aDir, bDir] = process.argv.slice(3).map(v => path.resolve(v));
  assert(aDir && bDir, 'compare requires two output directories');
  const a = readJson(path.join(aDir, 'results.json'));
  const b = readJson(path.join(bDir, 'results.json'));
  assert.equal(a.runtime.platform, b.runtime.platform);
  assert.equal(a.runtime.arch, b.runtime.arch);
  assert.equal(a.runtime.rayonThreads, b.runtime.rayonThreads);
  const { sharp } = setup(path.resolve(process.env.RESIZE_ROOT || ''));
  const rows = [];
  for (let i = 0; i < a.cases.length; i++) {
    const x = a.cases[i], y = b.cases[i];
    assert.equal(x.id, y.id);
    assert.equal(x.input.sha256, y.input.sha256);
    assert.deepEqual(x.resize, y.resize);
    const p = await normalized(sharp, fs.readFileSync(path.join(aDir, `${x.id}.png`)));
    const q = await normalized(sharp, fs.readFileSync(path.join(bDir, `${y.id}.png`)));
    assert.equal(p.width, q.width);
    assert.equal(p.height, q.height);
    let changedPixels = 0, changedChannels = 0, maxDelta = 0;
    for (let offset = 0; offset < p.data.length; offset += 4) {
      let changed = false;
      for (let channel = 0; channel < 4; channel++) {
        const delta = Math.abs(p.data[offset + channel] - q.data[offset + channel]);
        if (delta) { changed = true; changedChannels++; maxDelta = Math.max(maxDelta, delta); }
      }
      if (changed) changedPixels++;
    }
    rows.push({ id: x.id, dimensions: [p.width, p.height], pixelSha256: [p.sha256, q.sha256], alpha: [p.alpha, q.alpha], changedPixels, changedChannels, maxDelta,
      bytes: [x.output.bytes, y.output.bytes], coldWallMs: [x.cold.wallMs, y.cold.wallMs],
      warmWallMs: [x.warm.wallMs, y.warm.wallMs], warmOpsMs: [x.warm.opsMs, y.warm.opsMs],
      warmEncodeMs: [x.warm.encodeMs, y.warm.encodeMs], warmTotalMs: [x.warm.totalMs, y.warm.totalMs] });
    console.log(`${x.id} ${p.width}x${p.height} changedPixels=${changedPixels} maxDelta=${maxDelta} wall=${x.warm.wallMs.median.toFixed(2)}→${y.warm.wallMs.median.toFixed(2)}ms ops=${x.warm.opsMs.median.toFixed(2)}→${y.warm.opsMs.median.toFixed(2)}ms`);
  }
  fs.writeFileSync(path.join(bDir, 'comparison.json'), JSON.stringify({ baselineRevision: a.revision, candidateRevision: b.revision, runtime: b.runtime, cases: rows }, null, 2) + '\n');
}
(process.argv[2] === 'compare' ? compare() : run()).catch(err => { console.error(err); process.exitCode = 1; });
