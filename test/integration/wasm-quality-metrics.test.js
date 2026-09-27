const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const sharp = require('sharp');

async function main() {
  const { readChecked, rgbPsnr, score, sha256 } = await import('../benchmarks/wasm-quality-metrics.mjs');

  const black = { width: 1, height: 1, data: Uint8Array.from([0, 0, 0, 255]) };
  const changed = { width: 1, height: 1, data: Uint8Array.from([3, 4, 0, 0]) };
  assert.deepEqual(rgbPsnr(black, black), { status: 'perfect', value: 'Infinity' });
  assert.equal(JSON.stringify(rgbPsnr(black, black)), '{"status":"perfect","value":"Infinity"}');
  assert(Math.abs(rgbPsnr(black, changed).value - 10 * Math.log10(255 ** 2 / (25 / 3))) < 1e-12);
  assert.throws(() => rgbPsnr(black, { ...black, width: 2 }), /dimensions differ/);

  const png = await sharp({ create: { width: 16, height: 16, channels: 4,
    background: { r: 30, g: 80, b: 120, alpha: 1 } } }).png().toBuffer();
  const same = await score(png, png);
  assert.equal(same.status, 'measured');
  assert.equal(same.psnr.status, 'perfect');
  assert(Math.abs(same.ssim.value - 1) < 1e-10);
  assert.equal((await score(png, await sharp(png).resize(8, 8).png().toBuffer())).status, 'unmeasured');
  const bad = await score(png, Buffer.from('not an image'));
  assert.equal(bad.status, 'error');
  const statuses = JSON.parse(JSON.stringify([same.psnr,
    { status: 'unmeasured', value: null, reason: 'missing' }, bad.psnr]));
  assert.deepEqual(statuses.map((item) => item.status), ['perfect', 'unmeasured', 'error']);
  assert.equal(statuses[0].value, 'Infinity');

  const temp = await fs.mkdtemp(path.join(os.tmpdir(), 'wasm-quality-metrics-test-'));
  try {
    const input = path.join(temp, 'input.png');
    const output = path.join(temp, 'output.png');
    await fs.writeFile(input, png);
    await fs.writeFile(output, png);
    assert.deepEqual(await readChecked(input, sha256(png)), png);
    await assert.rejects(readChecked(input, '0'.repeat(64)), /SHA-256 mismatch/);
    await assert.rejects(readChecked(output, 'f'.repeat(64)), /SHA-256 mismatch/);
    await assert.rejects(readChecked(path.join(temp, 'missing.png'), sha256(png)), /ENOENT/);
  } finally { await fs.rm(temp, { recursive: true, force: true }); }

  console.log('Wasm quality metric checks passed');
}

main().catch((error) => { console.error(error); process.exitCode = 1; });
