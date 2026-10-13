// Exact decoded-file crops avoid adding the screenshot transport's JPEG losses.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [repository, phase, out] = process.argv.slice(2);
if (!repository || !phase || !out) throw Error('Usage: node crop-files.cjs <repository> <phase> <new-dir>');
const sharp = require(require.resolve('sharp', { paths: [path.resolve(repository)] }));
const sha = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const manifest = JSON.parse(fs.readFileSync(path.join(phase, 'runs/B1/manifest.json')));
fs.mkdirSync(out);
(async () => {
  const rows = [];
  for (const width of [684, 960, 1920]) for (const format of ['png', 'webp', 'jpeg']) {
    const parent = format === 'png' ? path.join(phase, 'controls', `control-${width}.png`)
      : path.join(phase, 'runs/B1', manifest.artifacts.find(a => a.width === width && a.format === format).path);
    const metadata = await sharp(parent).metadata();
    for (const [region, box] of Object.entries({ verification: [16, 465, 928, 318], failure: [16, 1208, 928, 145] })) {
      const pixels = { left: Math.floor(box[0] * metadata.width / 960), top: Math.floor(box[1] * metadata.height / 1400),
        width: Math.floor(box[2] * metadata.width / 960), height: Math.floor(box[3] * metadata.height / 1400) };
      const file = `${format}-${width}-${region}.png`;
      await sharp(parent).extract(pixels).png().toFile(path.join(out, file));
      rows.push({ file, source: format === 'png' ? `controls/control-${width}.png` : `runs/B1/${path.basename(parent)}`,
        sourceSha256: sha(parent), sourceFormat: metadata.format, encodedWidth: width, pixels, resize: false,
        sha256: sha(path.join(out, file)) });
    }
  }
  fs.writeFileSync(path.join(out, 'crops.json'), JSON.stringify({ scope: 'Decoded pixels at 1:1; lossless crop encoding, outside compiler timing and browser observation', decoder: sharp.versions, rows }, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ decodedFileCrops: rows.length, resize: false }));
})().catch(error => { console.error(error); process.exitCode = 1; });
