const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const [repository, history] = process.argv.slice(2);
if (!repository || !history) throw Error('Usage: node check-browser.cjs <repository> <saved-history>');
const sharp = require(require.resolve('sharp', { paths: [path.resolve(repository)] }));
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
const read = file => JSON.parse(fs.readFileSync(file));
(async () => {
  const dir = path.join(history, 'evidence/browser');
  const targets = { 'desktop-1': [1440, 1000, 1], 'desktop-2': [1440, 1000, 2], 'mobile-1': [390, 844, 1], 'mobile-2': [390, 844, 2] };
  const ids = ['svg', ...[684, 960, 1920].flatMap(width => ['png', 'webp', 'jpeg'].map(format => `${format}-${width}`)), 'Responsive B1'];
  const rows = fs.readdirSync(dir).filter(file => /^(desktop|mobile)-[12]-.*\.json$/.test(file)).map(file => ({ file, ...read(path.join(dir, file)) }));
  assert.equal(rows.length, 90);
  const pictures = [];
  for (const [context, target] of Object.entries(targets)) {
    const normal = rows.filter(row => row.case === context && row.page !== 'detail-svg');
    assert.deepEqual([...new Set(normal.map(row => row.page))].sort(), [...ids].sort());
    assert.equal(normal.length, context.startsWith('desktop') ? 22 : 11);
    const details = rows.filter(row => row.case === context && row.page === 'detail-svg');
    assert.equal(details.length, context.startsWith('desktop') ? 3 : 9);
    for (const row of [...normal, ...details]) {
      for (const state of [row.before, row.after]) assert.deepEqual([state.viewport.width, state.viewport.height, state.viewport.dpr], target);
      assert.equal(row.browserZoomPercent, 100); assert.equal(row.deviceToolbarZoomPercent, 75);
      assert.equal(row.qa.decodeError, null); assert.equal(row.before.images.length, 1);
      const image = row.before.images[0]; assert.equal(image.complete, true); assert.equal(image.decodeError, null);
      assert.equal(image.currentSrc, row.qa.images[0].currentSrc);
      const url = new URL(image.currentSrc); assert.equal(url.origin, 'http://127.0.0.1:8876');
      assert.ok(url.pathname.startsWith('/final/'));
      const file = path.join(history, 'preview', path.basename(url.pathname));
      assert.ok(fs.existsSync(file)); const encoded = await sharp(file).metadata();
      if (row.page !== 'detail-svg') assert.equal(image.rect.width, context.startsWith('desktop') ? 960 : 342);
      else assert.ok(row.before.clip && [960, 1920].includes(image.rect.width));
      const stem = row.file.slice(0, -5), crop = path.join(dir, 'captures', stem + '.png');
      const descriptor = read(crop + '.json'); assert.equal(hash(crop), descriptor.cropSha256);
      assert.equal(descriptor.sourceFormat, 'jpeg'); assert.equal(descriptor.pixelsPerCSSPixel, 0.75);
      if (row.page === 'Responsive B1' && row.part === 'top') {
        assert.equal(image.sizes, '(max-width: 1008px) calc(100vw - 48px), 960px');
        pictures.push({ case: context, file: path.basename(file), encodedWidth: encoded.width,
          naturalWidth: image.naturalWidth, cssWidth: image.rect.width, bytes: fs.statSync(file).size, sha256: hash(file) });
      }
    }
  }
  const summary = { uniqueNormalImageLoads: 44, enlargementPageLoads: 4, captureRecords: 90,
    normalCaptureRecords: 66, enlargementCaptureRecords: 24, decodeErrors: 0, pictures };
  const output = path.join(history, 'browser-check.json');
  if (process.argv[4] === '--check') assert.deepEqual(read(output), summary);
  else fs.writeFileSync(output, JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify(summary));
})().catch(error => { console.error(error); process.exitCode = 1; });
