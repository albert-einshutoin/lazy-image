// SVG rendering is diagnostic preparation, outside compileImage timing.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [repository, svg, out] = process.argv.slice(2);
if (!repository || !svg || !out) throw Error('Usage: node render.cjs <repository> <svg> <new-dir>');
const sharp = require(require.resolve('sharp', { paths: [path.resolve(repository)] }));
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
fs.mkdirSync(out);
(async () => {
  const input = path.join(out, 'diagram-input.png');
  await sharp(svg, { density: 144 }).flatten({ background: '#ffffff' }).removeAlpha().png().toFile(input);
  const metadata = await sharp(input).metadata();
  if (metadata.width !== 1920 || metadata.height !== 2800 || metadata.channels !== 3) throw Error('Unexpected raster input');
  const fonts = ['Arial.ttf', 'Arial Bold.ttf'].map(file => {
    const filename = path.join('/System/Library/Fonts/Supplemental', file);
    return { file, sha256: hash(filename) };
  });
  const report = { scope: 'External SVG rendering, not compiler performance', versions: sharp.versions,
    density: 144, background: '#ffffff', fontFamily: 'Arial', fonts,
    svg: { bytes: fs.statSync(svg).size, sha256: hash(svg) },
    input: { file: 'diagram-input.png', bytes: fs.statSync(input).size, sha256: hash(input), metadata } };
  fs.writeFileSync(path.join(out, 'render.json'), JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify(report));
})().catch(error => { console.error(error); process.exitCode = 1; });
