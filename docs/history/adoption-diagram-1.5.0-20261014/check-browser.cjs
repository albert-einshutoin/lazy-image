const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const [repository, history, originalPhase] = process.argv.slice(2);
if (!repository || !history || !originalPhase) throw Error('Usage: node check-browser.cjs <repository> <saved-history> <original-phase> [--check]');
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
    for (const id of ids) assert.deepEqual(normal.filter(row => row.page === id).map(row => row.part).sort(), context.startsWith('desktop') ? ['bottom', 'top'] : ['top']);
    const details = rows.filter(row => row.case === context && row.page === 'detail-svg');
    assert.equal(details.length, context.startsWith('desktop') ? 3 : 9);
    const poses = context.startsWith('desktop') ? ['failure', 'verify', 'zoom2-verify']
      : ['verify-left', 'verify-middle', 'verify-right', 'verify-end', 'failure-left', 'failure-middle', 'failure-right', 'failure-end', 'zoom2-verify'];
    assert.deepEqual(details.map(row => row.pose).sort(), poses.sort());
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
      else {
        assert.ok(row.before.clip);
        const zoomed = row.pose === 'zoom2-verify';
        assert.equal(image.rect.width, zoomed ? 1920 : 960);
        const horizontal = { left: 0, middle: 272, right: 544, end: 620 };
        const expectedLeft = context.startsWith('desktop') || zoomed ? 0 : horizontal[row.pose.split('-')[1]];
        const expectedTop = zoomed ? 944 : row.pose.startsWith('verify') ? 472 : context.startsWith('desktop') ? 620 : 776;
        for (const state of [row.before, row.after]) assert.deepEqual(state.pan, { left: expectedLeft, top: expectedTop });
      }
      const stem = row.file.slice(0, -5), crop = path.join(dir, 'captures', stem + '.png');
      const descriptor = read(crop + '.json'); assert.equal(hash(crop), descriptor.cropSha256);
      assert.equal(descriptor.captureRecordSha256, hash(path.join(dir, row.file)));
      const original = path.join(originalPhase, 'browser/private-native', stem + '.jpg');
      const originalRecord = path.join(originalPhase, 'browser', row.file);
      assert.equal(hash(originalRecord), descriptor.captureRecordSha256);
      assert.equal(hash(original), descriptor.originalSha256);
      const originalMetadata = await sharp(original).metadata(), cropMetadata = await sharp(crop).metadata();
      assert.deepEqual(descriptor.originalPixels, { width: originalMetadata.width, height: originalMetadata.height });
      assert.equal(cropMetadata.width, descriptor.cropPixels.width); assert.equal(cropMetadata.height, descriptor.cropPixels.height);
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
  if (process.argv[5] === '--check') assert.deepEqual(read(output), summary);
  else fs.writeFileSync(output, JSON.stringify(summary, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify(summary));
})().catch(error => { console.error(error); process.exitCode = 1; });
