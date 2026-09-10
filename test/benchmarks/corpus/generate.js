// Regenerates project-owned synthetic fixtures; the two CC0 photographs are tracked originals.
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const { sha256 } = require('../../helpers/benchmark-corpus');
const { resolveRoot } = require('../../helpers/paths');
const { createRgbaPng } = require('../../helpers/png-helpers');
const dir = path.join(__dirname, 'images');
const policy = { preset: 'publicUpload', widths: [160], formats: ['jpeg', 'webp', 'avif'],
  budgets: ['jpeg','webp','avif'].map(format => ({ width: 160, format, maxBytes: 24000 })), placeholder: false };
async function main() {
  const manifest = JSON.parse(fs.readFileSync(path.join(__dirname, 'manifest.json')));
  manifest.name = 'release-evidence-v1';
  manifest.additionalLicenses = [{ spdx: 'CC0-1.0', path: 'test/benchmarks/corpus/CC0-1.0.txt',
    sha256: sha256(fs.readFileSync(path.join(__dirname, 'CC0-1.0.txt'))) }];
  function entry(id, ext, category, expectations = {}, source = null) {
    const relative = `test/benchmarks/corpus/images/${id}.${ext}`;
    const data = fs.readFileSync(resolveRoot(relative));
    manifest.entries.push({ id, path: relative, bytes: data.length, sha256: sha256(data),
      license: source ? 'CC0-1.0' : 'MIT', category,
      source: source || { kind: 'project-generated-fixture', generator: 'test/benchmarks/corpus/generate.js' },
      expectations: { inputFormat: ext === 'jpg' ? 'jpeg' : ext, ...expectations }, policy: structuredClone(policy) });
  }
  for (const [id, author] of [['coffee','Rachel Michetti'], ['chelsea','Stefan van der Walt']]) {
    entry(id, 'png', 'photo', {}, { kind: 'photograph', author,
      url: `https://raw.githubusercontent.com/scikit-image/scikit-image/v0.19.3/skimage/data/${id}.png`,
      licenseSource: 'https://github.com/scikit-image/scikit-image/blob/v0.25.2/skimage/data/_fetchers.py',
      note: 'CC0 dedication documented by scikit-image; original pixels, no local transformation.' });
  }
  for (const n of [1, 2]) {
    const bg = n === 1 ? '#f6f7fa' : '#151826';
    const fg = n === 1 ? '#14243a' : '#e8edf5';
    const svg = `<svg width="320" height="240" xmlns="http://www.w3.org/2000/svg"><rect width="320" height="240" fill="${bg}"/><rect x="12" y="12" width="296" height="32" fill="#437ace"/><g fill="${fg}" font-family="sans-serif" font-size="14"><text x="20" y="70">Image upload ${n}</text><text x="20" y="100">JPEG WebP AVIF</text><text x="20" y="130">24000 bytes / verified</text></g><path d="M20 155H300M20 180H300M20 205H300" stroke="${fg}"/></svg>`;
    await sharp(Buffer.from(svg)).removeAlpha().png().toFile(path.join(dir, `ui-${n}.png`));
    entry(`ui-${n}`, 'png', 'ui', { note: 'Synthetic UI text/line workload; not an application capture.' });
    const art = `<svg width="320" height="240" xmlns="http://www.w3.org/2000/svg"><defs><linearGradient id="g"><stop stop-color="#f85"/><stop offset="1" stop-color="#35e"/></linearGradient></defs><rect width="320" height="240" fill="${n === 1 ? 'url(#g)' : '#e9f5ec'}"/><circle cx="100" cy="110" r="70" fill="#387e67"/><path d="M180 40L300 210H100Z" fill="#f8b442"/></svg>`;
    await sharp(Buffer.from(art)).removeAlpha().png().toFile(path.join(dir, `illustration-${n}.png`));
    entry(`illustration-${n}`, 'png', 'illustration');
    const rgba = Buffer.alloc(160 * 120 * 4);
    for (let y=0;y<120;y++) for (let x=0;x<160;x++) {
      const i=(y*160+x)*4; rgba[i]=x; rgba[i+1]=y*2; rgba[i+2]=180;
      rgba[i+3]=n===1 ? Math.round(x/159*255) : ((x-80)**2+(y-60)**2 < 40**2 ? 128 : 0);
    }
    const ext=n===1?'png':'webp';
    await sharp(rgba,{raw:{width:160,height:120,channels:4}}).toFormat(ext,{lossless:true}).toFile(path.join(dir,`alpha-${n}.${ext}`));
    entry(`alpha-${n}`,ext,'alpha',{outputAlpha:true, maxAlphaDelta:8});
    manifest.entries.at(-1).policy.formats=['webp','avif'];
    manifest.entries.at(-1).policy.budgets=manifest.entries.at(-1).policy.budgets.filter(b=>b.format!=='jpeg');
    await sharp(path.join(dir,`ui-${n}.png`)).withMetadata({orientation:n===1?6:3}).withIccProfile('srgb')
      .withExifMerge({IFD0:{Artist:'Synthetic benchmark'}, IFD3:{GPSLatitudeRef:'N',GPSLatitude:'1/1 0/1 0/1',GPSLongitudeRef:'E',GPSLongitude:'1/1 0/1 0/1'}})
      .jpeg({quality:95}).toFile(path.join(dir,`metadata-${n}.jpg`));
    entry(`metadata-${n}`,'jpg','metadata',{exif:true,icc:true,orientation:n===1?6:3});
    // Valid PNGs whose decoded dimensions/pixel count exceed public-upload limits.
    fs.writeFileSync(path.join(dir,`hostile-${n}.png`),createRgbaPng(n===1?32769:6400,n===1?512:6400));
    entry(`hostile-${n}`,'png','hostile',{expectedError:n===1?'E400':'E123', expectedPhase:n===1?'planning':'preflight',
      ...(n===1 ? {expectedCause:'source.width must be an integer between 1 and 32768'} : {})});
    manifest.entries.at(-1).policy.formats=['webp'];
    manifest.entries.at(-1).policy.budgets=manifest.entries.at(-1).policy.budgets.filter(b=>b.format==='webp');
  }
  fs.writeFileSync(path.join(__dirname,'release-manifest.json'),JSON.stringify(manifest,null,2)+'\n');
}
main().catch(error=>{console.error(error);process.exitCode=1;});
