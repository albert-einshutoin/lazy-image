const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const [repository, destination] = process.argv.slice(2);
if (!repository || !destination || fs.existsSync(destination)) throw Error('Usage: node prepare.cjs <repository> <new-preview-directory>');
const old = path.join(repository, 'docs/history/blog-adoption-1.5.0-20261013/preview');
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
fs.cpSync(old, destination, { recursive: true });
for (const name of ['single-webp.html', 'single-png.html']) fs.copyFileSync(path.join(__dirname, 'fixtures', name), path.join(destination, name));
fs.copyFileSync(path.join(__dirname, 'diagnose.js'), path.join(destination, 'diagnose.js'));
const webp = fs.readFileSync(path.join(old, 'desktop-A/images/metadata-1/image-240.webp'));
const png = fs.readFileSync(path.join(old, 'reference/coffee-64.png'));
for (const name of ['quality', 'quality-fresh']) {
  const html = fs.readFileSync(path.join(old, name + '.html'), 'utf8')
    .replace('alt="metadata-1 A"', `alt="metadata-1 A" data-probe-sha="${hash(webp)}"`)
    .replace('alt="coffee-64 Original"', `alt="coffee-64 Original" data-probe-sha="${hash(png)}"`)
    .replace('<script src="qa.js"></script>', '')
    // Register load/error listeners before the parser starts image requests.
    .replace('<title>', '<script src="diagnose.js"></script><title>')
    .replace('id="qa-observation"', 'id="decode-observation"');
  fs.writeFileSync(path.join(destination, 'diagnostic-' + name + '.html'), html, { flag: 'wx' });
}
// Controlled truncation is a negative fixture, never a replacement for saved bytes.
const broken = png.subarray(0, 64); fs.writeFileSync(path.join(destination, 'broken.png'), broken, { flag: 'wx' });
const invalid = Buffer.from('This is intentionally not an image.\n');
fs.writeFileSync(path.join(destination, 'invalid.png'), invalid, { flag: 'wx' });
for (const [name, bad, expected] of [['broken', 'broken.png', broken], ['missing', 'absent-control.png', broken], ['invalid', 'invalid.png', invalid]]) {
  const imgs = [
    `<img alt="metadata-1 A" src="desktop-A/images/metadata-1/image-240.webp" data-probe-sha="${hash(webp)}">`,
    `<img alt="coffee-64 Original" src="reference/coffee-64.png" data-probe-sha="${hash(png)}">`,
    `<img alt="control-${name}" src="${bad}" data-probe-sha="${hash(expected)}">`,
  ].join('\n');
  fs.writeFileSync(path.join(destination, `control-${name}.html`), `<!doctype html><html><meta charset="utf-8"><script src="diagnose.js"></script><title>${name} negative control</title><h1>${name} negative control</h1>${imgs}<pre id="decode-observation">loading</pre></html>`, { flag: 'wx' });
}
console.log(JSON.stringify({ scope: 'Stored bytes copied; compiler not executed', diagnosticPages: 2, controlPages: 3 }));
