// Crop only the owned diagram from a private native-window capture; keep scale explicit.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [repository, original, recordFile, out] = process.argv.slice(2);
if (!repository || !original || !recordFile || !out) throw Error('Usage: node crop-native.cjs <repository> <native-capture> <capture-json> <new-crop-png>');
if (fs.existsSync(out)) throw Error('Crop destination exists');
const sharp = require(require.resolve('sharp', { paths: [path.resolve(repository)] }));
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
(async () => {
  const record = JSON.parse(fs.readFileSync(recordFile));
  if (JSON.stringify(record.before.viewport) !== JSON.stringify(record.after.viewport)) throw Error('Capture changed viewport/DPR');
  if (record.before.images.length !== 1 || record.before.images[0].decodeError !== null) throw Error('Expected one decoded image');
  const metadata = await sharp(original).metadata();
  const { data, info } = await sharp(original).removeAlpha().raw().toBuffer({ resolveWithObject: true });
  if (info.channels !== 3) throw Error('Unexpected native screenshot channels');
  const runs = [];
  for (let y = 0; y < info.height; y++) {
    let start = -1;
    for (let x = 0; x <= info.width; x++) {
      const offset = (y * info.width + x) * 3;
      const black = x < info.width && data[offset] < 20 && data[offset + 1] < 20 && data[offset + 2] < 20;
      if (black && start < 0) start = x;
      if (!black && start >= 0) { if (x - start >= 50 && x - start <= 110) runs.push({ x: start, y, width: x - start }); start = -1; }
    }
  }
  if (!runs.length) throw Error('Known 100 CSS px calibration ruler not found');
  const counts = new Map();
  for (const run of runs) { const key = `${run.x}/${run.width}`; counts.set(key, (counts.get(key) || 0) + 1); }
  const candidates = [...counts].filter(([, count]) => count >= 8);
  if (candidates.length !== 1) throw Error('Ambiguous calibration ruler');
  const [x, width] = candidates[0][0].split('/').map(Number);
  const matching = runs.filter(run => run.x === x && run.width === width);
  if (matching.at(-1).y - matching[0].y > 24) throw Error('Calibration is not one compact rectangle');
  const ruler = { x, y: matching[0].y, width };
  const scale = ruler.width / record.before.ruler.width;
  if (Math.abs(scale - 0.75) > 0.02) throw Error('Unexpected presentation scale');
  const origin = { x: ruler.x - record.before.ruler.x * scale, y: ruler.y - record.before.ruler.y * scale };
  const rect = record.before.images[0].rect, viewport = record.before.viewport;
  let left = Math.max(0, rect.x), top = Math.max(0, rect.y);
  let right = Math.min(viewport.width, rect.x + rect.width), bottom = Math.min(viewport.height, rect.y + rect.height);
  if (record.page.startsWith('detail-')) {
    const clip = record.before.clip;
    if (!clip || clip.width <= 0 || clip.height <= 0) throw Error('Enlargement requires the observed scroll-region bounds');
    left = Math.max(left, clip.x); top = Math.max(top, clip.y);
    right = Math.min(right, clip.x + clip.width); bottom = Math.min(bottom, clip.y + clip.height);
  }
  if (right <= left || bottom <= top) throw Error('Image is outside visible viewport');
  const pixels = { left: Math.ceil(origin.x + left * scale), top: Math.ceil(origin.y + top * scale),
    width: Math.floor((right - left) * scale), height: Math.floor((bottom - top) * scale) };
  if (pixels.left < 0 || pixels.top < 0 || pixels.left + pixels.width > info.width || pixels.top + pixels.height > info.height) throw Error('Crop exceeds screenshot');
  await sharp(original).extract(pixels).png().toFile(out);
  const result = { scope: 'Visible diagram only; native device-toolbar presentation is scaled, not a 1:1 source-pixel screenshot',
    method: 'cua App.getScreenshot, unchanged viewport/DPR, measured 100 CSS px ruler; PNG crop retains original capture compression',
    sourceFormat: metadata.format, publicFormat: 'png', scaleUncertaintyPixels: 2,
    originalSha256: sha(original), captureRecordSha256: sha(recordFile), originalPixels: { width: info.width, height: info.height },
    rulerPixels: ruler, pixelsPerCSSPixel: scale, cropPixels: pixels,
    cssVisibleBounds: { left, top, right, bottom }, cropSha256: sha(out) };
  fs.writeFileSync(out + '.json', JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ crop: path.basename(out), pixelsPerCSSPixel: scale, pixels }));
})().catch(error => { console.error(error); process.exitCode = 1; });
