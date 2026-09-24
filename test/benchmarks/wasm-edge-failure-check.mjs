import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import sharp from 'sharp';
import { runEdgeWorkerd } from './wasm-edge-workerd.mjs';

const registry = 'https://registry.npmjs.org/';
const codecFiles = {
  'mozjpeg_dec.wasm': ['@jsquash/jpeg', 'codec/dec/mozjpeg_dec.wasm'],
  'mozjpeg_enc.wasm': ['@jsquash/jpeg', 'codec/enc/mozjpeg_enc.wasm'],
  'squoosh_png_bg.wasm': ['@jsquash/png', 'codec/pkg/squoosh_png_bg.wasm'],
  'squoosh_resize_bg.wasm': ['@jsquash/resize', 'lib/resize/pkg/squoosh_resize_bg.wasm'],
  'webp_enc.wasm': ['@jsquash/webp', 'codec/enc/webp_enc.wasm'],
};

const version = process.argv[2] ?? '1.3.1';
assert(/^\d+\.\d+\.\d+$/.test(version), 'pass an exact published version');
const directory = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-edge-negative-'));
try {
  await fs.writeFile(path.join(directory, 'package.json'), '{"private":true,"type":"module"}');
  const install = spawnSync('npm', ['install', '--registry', registry, '--save-exact',
    `@alberteinshutoin/lazy-image-wasm@${version}`, 'esbuild@0.25.10', 'workerd@1.20260924.1'],
  { cwd: directory, encoding: 'utf8' });
  assert.equal(install.status, 0, install.stderr);
  const installedWorkerd = path.join(directory, 'node_modules/workerd/bin/workerd');
  const packageInfo = { packageDir: path.join(directory, 'node_modules/@alberteinshutoin/lazy-image-wasm'),
    toolchainBinaries: { workerd: { package: 'workerd',
      sha256: createHash('sha256').update(await fs.readFile(installedWorkerd)).digest('hex') } } };
  const wrongWorkerd = path.join(directory, 'wrong-workerd');
  await fs.writeFile(wrongWorkerd, '#!/bin/sh\nexit 0\n', { mode: 0o755 });
  const badImage = path.join(directory, 'bad-image.jpg');
  await fs.writeFile(badImage, 'not a jpeg');
  const validateOutput = async (bytes, result, item, outputPath) => {
    const metadata = await sharp(bytes).metadata();
    await sharp(bytes).raw().toBuffer();
    assert.equal(metadata.format, item.options.format);
    assert.equal(bytes.length, result.bytesOut);
    await fs.writeFile(outputPath, bytes);
    return { bytes: bytes.length, format: metadata.format, width: metadata.width,
      height: metadata.height, metadata: { exif: false, xmp: false, icc: false } };
  };
  const checks = [
    { name: 'runtime-unavailable', expected: 'BLOCKED',
      options: { workerdPath: '/definitely/missing/workerd', codecFiles, cases: [] } },
    { name: 'runtime-override-mismatch', expected: 'FAIL',
      options: { workerdPath: wrongWorkerd, codecFiles, cases: [] } },
    { name: 'wasm-module-missing', expected: 'FAIL',
      options: { codecFiles: Object.fromEntries(Object.entries(codecFiles)
        .filter(([name]) => name !== 'webp_enc.wasm')), cases: [] } },
    { name: 'image-processing-failure', expected: 'FAIL',
      options: { codecFiles, cases: [{ id: 'invalid-jpeg', input: badImage,
        options: { format: 'webp', maxWidth: 320, maxHeight: 320, targetBytes: 50000,
          minQuality: 45, maxQuality: 86, qualityFloorPolicy: 'best-effort', output: 'arrayBuffer' } }] } },
  ];
  for (const check of checks) {
    for (const name of Object.keys(codecFiles)) {
      await fs.rm(path.join(directory, name), { force: true });
    }
    let caught;
    try {
      await runEdgeWorkerd({ directory, packageInfo, outputDir: directory, validateOutput, ...check.options });
    } catch (error) { caught = error; }
    assert(caught?.partialEdgeResults, `${check.name}: false PASS or missing failure evidence`);
    assert.equal(caught.partialEdgeResults.status, check.expected, caught.message);
    console.log(JSON.stringify({ name: check.name, verdict: check.expected, reason: caught.message,
      failedCases: caught.partialEdgeResults.results.filter((entry) => entry.status === 'FAIL') }));
  }
} finally {
  await fs.rm(directory, { recursive: true, force: true });
}
