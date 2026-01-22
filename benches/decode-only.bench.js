const { performance } = require('perf_hooks');
const path = require('path');
const fs = require('fs');
const { ImageEngine } = require('../index');

// Decode-only benchmark to surface decoder速度単体の差分
// Usage: node benches/decode-only.bench.js
async function run() {
  const fixtureDir = path.join(__dirname, '..', 'test', 'fixtures');
  const files = [
    'test_100KB_1188x1188.png',
    'test_90KB_1471x1471.webp',
    'test_105KB_1057x1057.jpg',
  ];

  console.log('=== Decode-only benchmark (no ops, no re-encode) ===');
  for (const file of files) {
    const full = path.join(fixtureDir, file);
    const data = fs.readFileSync(full);
    const start = performance.now();
    const img = ImageEngine.from(data);
    // decode only: dimensions() will force decode but avoid encode cost
    const dims = img.dimensions();
    const elapsed = performance.now() - start;
    console.log(`${file}: ${dims.width}x${dims.height} -> ${elapsed.toFixed(2)} ms`);
  }
}

run().catch((e) => {
  console.error(e);
  process.exit(1);
});
