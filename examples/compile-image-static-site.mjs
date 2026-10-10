/**
 * Compile one image and consume its manifest in a static page.
 * node examples/compile-image-static-site.mjs <input> <new-site-dir> [policy.json]
 * node examples/compile-image-static-site.mjs --self-test
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { fixturePath, loadLazyImage, makeTempDir, removeDir } from './_load.mjs';

const script = fileURLToPath(import.meta.url);
const args = process.argv.slice(2);

try {
  if (args.length === 1 && args[0] === '--self-test') {
    await runSelfTest();
  } else {
    if (args.length < 2 || args.length > 3) {
      throw new Error('Usage: node compile-image-static-site.mjs <input> <new-site-dir> [policy.json]');
    }
    const policy = args[2] ? JSON.parse(fs.readFileSync(args[2], 'utf8'))
      : { widths: [320, 640, 960], formats: ['webp'], placeholder: true };
    await buildSite(args[0], args[1], policy);
  }
} catch (error) {
  console.error(`FAIL [${error.phase || 'build'}/${error.errorCode || error.code || 'unknown'}]: ${error.message}`);
  if (error.recoveryHint) console.error(error.recoveryHint);
  if (error.cause) console.error(`Cause: ${error.cause.message || String(error.cause)}`);
  if (error.cleanupError) console.error(`Cleanup failed: ${error.cleanupError.message}`);
  process.exitCode = 1;
}

async function buildSite(inputPath, siteDir, policy) {
  // Claim only a new site directory; an existing site is never a retry target.
  fs.mkdirSync(siteDir);
  const { compileImage } = loadLazyImage();
  const manifest = await compileImage({ inputPath, outputDir: path.join(siteDir, 'images'), policy });
  const imageUrl = relative => `images/${relative.split('/').map(encodeURIComponent).join('/')}`;
  // Picture uses the first supported source; reserve JPEG for the img fallback.
  const formats = ['avif', 'webp', 'jpeg'].filter(format => manifest.artifacts.some(artifact => artifact.format === format));
  const variants = format => manifest.artifacts.filter(artifact => artifact.format === format);
  const srcset = format => variants(format).map(artifact => `${imageUrl(artifact.path)} ${artifact.width}w`).join(', ');
  const fallbackFormat = formats.at(-1);
  const fallback = variants(fallbackFormat).at(-1);
  const displayWidth = Math.min(960, fallback.width);
  const sizes = `(max-width: ${displayWidth + 48}px) calc(100vw - 48px), ${displayWidth}px`;
  const placeholder = manifest.placeholder
    ? ` style="background-image: url('${imageUrl(manifest.placeholder.path)}'); background-size: cover"` : '';
  const html = `<!doctype html>
<html lang="en">
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>compileImage static build</title>
<style>
  body { margin: 0; color: #18232d; background: #f4f6f8; font: 16px/1.5 system-ui, sans-serif; }
  main { max-width: 960px; margin: 32px auto; padding: 24px; }
  img { display: block; max-width: 100%; height: auto; border-radius: 12px; }
</style>
<main>
  <h1>compileImage static build</h1>
  <p>Actual widths: ${[...new Set(manifest.artifacts.map(artifact => artifact.width))].join(', ')} px. No upscaling.</p>
  <picture>
${formats.slice(0, -1).map(format => `    <source type="image/${format}" srcset="${srcset(format)}" sizes="${sizes}">\n`).join('')}    <img src="${imageUrl(fallback.path)}" srcset="${srcset(fallbackFormat)}" sizes="${sizes}" width="${fallback.width}" height="${fallback.height}" alt="Compiled example image"${placeholder}>
  </picture>
  <p><a href="images/manifest.json">Compiler manifest</a></p>
</main>
</html>
`;
  // HTML is an application output after the image commit, not a site transaction.
  fs.writeFileSync(path.join(siteDir, 'index.html'), html, { flag: 'wx' });
  console.log(`Built ${path.join(siteDir, 'index.html')} using ${manifest.artifacts.length} verified artifacts`);
}

async function runSelfTest() {
  const temporary = makeTempDir('lazy-image-static-site-');
  const input = fixturePath('test_100KB_1057x1057.jpg');
  const run = (...parameters) => spawnSync(process.execPath, [script, ...parameters], { encoding: 'utf8' });
  try {
    const site = path.join(temporary, 'normal');
    const normal = run(input, site);
    assert.equal(normal.status, 0, normal.stderr);
    checkPage(site, [320, 640, 960]);

    const original = snapshot(site);
    const repeat = run(input, site);
    assert.equal(repeat.status, 1);
    assert.match(repeat.stderr, /EEXIST/);
    assert.deepEqual(snapshot(site), original, 'existing site must remain byte-identical');

    const smallInput = path.join(temporary, 'small.jpg');
    const { ImageEngine } = loadLazyImage();
    await ImageEngine.fromPath(input).resize({ width: 64 }).toFile(smallInput, 'jpeg', 80);
    const smallSite = path.join(temporary, 'small');
    const small = run(smallInput, smallSite);
    assert.equal(small.status, 0, small.stderr);
    checkPage(smallSite, [64]);

    const invalid = path.join(temporary, 'invalid.jpg');
    fs.writeFileSync(invalid, 'not an image');
    const invalidSite = path.join(temporary, 'invalid');
    const rejection = run(invalid, invalidSite);
    assert.equal(rejection.status, 1);
    assert.match(rejection.stderr, /preflight/);
    assert.equal(fs.existsSync(path.join(invalidSite, 'index.html')), false);
    assert.equal(fs.existsSync(path.join(invalidSite, 'images')), false);

    const policy = path.join(temporary, 'strict.json');
    fs.writeFileSync(policy, JSON.stringify({ widths: [320], formats: ['webp'], placeholder: true,
      budgets: [{ width: 320, format: 'webp', maxBytes: 1 }] }));
    const strictSite = path.join(temporary, 'strict');
    const strict = run(input, strictSite, policy);
    assert.equal(strict.status, 1);
    assert.match(strict.stderr, /processing\/E300/);
    assert.equal(fs.existsSync(path.join(strictSite, 'index.html')), false);
    assert.equal(fs.existsSync(path.join(strictSite, 'images')), false);

    const formatsPolicy = path.join(temporary, 'formats.json');
    fs.writeFileSync(formatsPolicy, JSON.stringify({ widths: [320], formats: ['jpeg', 'webp'], placeholder: false }));
    const pictureSite = path.join(temporary, 'picture');
    const picture = run(input, pictureSite, formatsPolicy);
    assert.equal(picture.status, 0, picture.stderr);
    const pictureManifest = JSON.parse(fs.readFileSync(path.join(pictureSite, 'images', 'manifest.json')));
    const pictureHtml = fs.readFileSync(path.join(pictureSite, 'index.html'), 'utf8');
    const webp = pictureManifest.artifacts.find(artifact => artifact.format === 'webp');
    const jpeg = pictureManifest.artifacts.find(artifact => artifact.format === 'jpeg');
    assert.match(pictureHtml, /<source type="image\/webp"/);
    assert.ok(!pictureHtml.includes('<source type="image/jpeg"'));
    assert.ok(pictureHtml.includes(`srcset="images/${webp.path} ${webp.width}w"`));
    assert.ok(pictureHtml.includes(`<img src="images/${jpeg.path}"`));
    console.log('PASS: normal, no-upscale, invalid input, strict budget, existing site protection and modern picture source preference');
  } finally {
    removeDir(temporary);
  }
}

function checkPage(site, widths) {
  const manifest = JSON.parse(fs.readFileSync(path.join(site, 'images', 'manifest.json')));
  const html = fs.readFileSync(path.join(site, 'index.html'), 'utf8');
  assert.deepEqual(manifest.artifacts.map(artifact => artifact.width), widths);
  assert.ok(manifest.placeholder);
  for (const artifact of [...manifest.artifacts, manifest.placeholder]) {
    const bytes = fs.readFileSync(path.join(site, 'images', artifact.path));
    assert.equal(bytes.length, artifact.bytes);
    assert.equal(createHash('sha256').update(bytes).digest('hex'), artifact.sha256);
  }
  const srcset = html.match(/srcset="([^"]+)"/);
  assert.ok(srcset, 'page must contain a responsive image');
  assert.deepEqual(srcset[1].split(', ').map(value => Number(value.match(/ (\d+)w$/)[1])), widths);
  if (widths.length === 1 && widths[0] === 64) {
    assert.match(html, /sizes="[^"]*, 64px"/);
    assert.match(html, /width="64" height="64"/);
  }
  for (const match of html.matchAll(/(?:src|href)="([^"]+)"|url\('([^']+)'\)/g)) {
    const relative = match[1] || match[2];
    assert.ok(relative.startsWith('images/'), 'page URLs must be site-relative');
    assert.ok(fs.existsSync(path.join(site, decodeURIComponent(relative))), relative);
  }
  assert.ok(!html.includes(site));
  assert.ok(!html.includes(fixturePath('test_100KB_1057x1057.jpg')));
}

function snapshot(directory) {
  return Object.fromEntries(fs.readdirSync(directory).sort().flatMap(name => {
    const file = path.join(directory, name);
    if (fs.statSync(file).isDirectory()) {
      return Object.entries(snapshot(file)).map(([child, hash]) => [path.join(name, child), hash]);
    }
    return [[name, createHash('sha256').update(fs.readFileSync(file)).digest('hex')]];
  }));
}
