/**
 * Benchmark: resize(2000x2000) + crop(800x600) lazy-image (fused Extract) vs sharp (separate).
 */

const fs = require('fs');
const path = require('path');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));

let sharp;
try {
    sharp = require('sharp');
} catch (e) {
    console.error('❌ sharp is not installed. Run `npm install sharp` to enable this benchmark.');
    process.exit(1);
}

const SOURCE = resolveFixture('test_4.5MB_5000x5000.png');
const OUTPUT = resolveTemp('benchmarks', 'extract-comparison');
if (!fs.existsSync(OUTPUT)) {
    fs.mkdirSync(OUTPUT, { recursive: true });
}

const TARGET_W = 2000;
const TARGET_H = 2000;
const CROP = { left: 100, top: 100, width: 800, height: 600 };

function formatBytes(bytes) {
    return `${(bytes / 1024 / 1024).toFixed(2)} MB`;
}

async function benchLazy() {
    const start = Date.now();
    const { data, metrics } = await ImageEngine.fromPath(SOURCE)
        .resize(TARGET_W, TARGET_H)
        .crop(CROP.left, CROP.top, CROP.width, CROP.height)
        .toBufferWithMetrics('png');
    const elapsed = Date.now() - start;
    fs.writeFileSync(path.join(OUTPUT, 'lazy.png'), data);
    return { time: elapsed, size: data.length, peakRss: metrics.peakRss };
}

async function benchSharp() {
    const start = Date.now();
    const buffer = await sharp(SOURCE)
        .resize(TARGET_W, TARGET_H, { fit: 'cover' })
        .extract(CROP)
        .png()
        .toBuffer();
    const elapsed = Date.now() - start;
    fs.writeFileSync(path.join(OUTPUT, 'sharp.png'), buffer);
    // Sharp does not expose peak RSS; approximate with process RSS delta
    const peakRss = process.memoryUsage().rss;
    return { time: elapsed, size: buffer.length, peakRss };
}

async function main() {
    console.log('=== Extract Benchmark: lazy-image vs sharp ===');
    console.log(`Source: ${SOURCE}`);
    const stats = fs.statSync(SOURCE);
    console.log(`Input size: ${formatBytes(stats.size)}`);
    console.log(`Ops: resize(${TARGET_W}x${TARGET_H}) -> crop(${CROP.width}x${CROP.height} at ${CROP.left},${CROP.top})\n`);

    const lazy = await benchLazy();
    console.log(`lazy-image: ${lazy.time}ms, ${formatBytes(lazy.size)}, peakRss ${formatBytes(lazy.peakRss)}`);

    const sharpResult = await benchSharp();
    console.log(`sharp:      ${sharpResult.time}ms, ${formatBytes(sharpResult.size)}, rss ${formatBytes(sharpResult.peakRss)}`);

    const speedDiff = ((sharpResult.time - lazy.time) / sharpResult.time) * 100;
    const sizeDiff = ((lazy.size - sharpResult.size) / sharpResult.size) * 100;

    console.log('\n--- Summary ---');
    console.log(`Speed: ${speedDiff >= 0 ? '+' : ''}${speedDiff.toFixed(1)}% vs sharp (positive = faster)`);
    console.log(`Size:  ${sizeDiff >= 0 ? '+' : ''}${sizeDiff.toFixed(1)}% vs sharp (positive = larger)`);
    console.log('Outputs saved to:', OUTPUT);
}

main().catch((err) => {
    console.error('Benchmark failed:', err);
    process.exit(1);
});
