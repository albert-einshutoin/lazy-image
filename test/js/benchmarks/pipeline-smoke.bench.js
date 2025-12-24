// pipeline-smoke.bench.js - quick benchmark between lazy-image and sharp
const fs = require('fs');
const path = require('path');
const { resolveFixture, resolveRoot, TEST_DIR } = require('../helpers/paths');
const { ImageEngine, version, supportedOutputFormats } = require(resolveRoot('index'));

// Optional: compare with sharp if installed
let sharp = null;
try {
  sharp = require('sharp');
} catch {
  console.log('⚠️  sharp not installed, skipping comparison');
}

async function benchmark(name, fn) {
  const start = process.hrtime.bigint();
  const result = await fn();
  const end = process.hrtime.bigint();
  const ms = Number(end - start) / 1_000_000;
  return { name, result, ms };
}

async function main() {
  console.log('='.repeat(60));
  console.log(`lazy-image v${version()}`);
  console.log(`Supported formats: ${supportedOutputFormats().join(', ')}`);
  console.log('='.repeat(60));

  const inputPath = resolveFixture('test_input.png');
  if (!fs.existsSync(inputPath)) {
    console.error('❌ test_input.png not found');
    process.exit(1);
  }

  const outputDir = path.join(TEST_DIR, 'output', 'benchmarks', 'pipeline-smoke');
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  const inputBuf = fs.readFileSync(inputPath);
  console.log(`Input: ${inputPath} (${(inputBuf.length / 1024).toFixed(1)} KB)`);
  console.log('');

  // ============================================================================
  // TEST 1: JPEG encoding with mozjpeg
  // ============================================================================
  console.log('📸 Test 1: JPEG encoding (quality=75)');
  console.log('-'.repeat(40));

  // Rust/mozjpeg
  const rustJpeg = await benchmark('lazy-image', async () => {
    return ImageEngine.from(inputBuf)
      .resize(800, null)
      .toBuffer('jpeg', 75);
  });

  fs.writeFileSync(path.join(outputDir, 'out_rust.jpg'), rustJpeg.result);
  console.log(`  lazy-image: ${rustJpeg.ms.toFixed(1)}ms, ${rustJpeg.result.length} bytes`);

  // Sharp comparison
  if (sharp) {
    const sharpJpeg = await benchmark('sharp', async () => {
      return sharp(inputBuf)
        .resize(800)
        .jpeg({ quality: 75, mozjpeg: true })
        .toBuffer();
    });

    fs.writeFileSync(path.join(outputDir, 'out_sharp.jpg'), sharpJpeg.result);
    console.log(`  sharp:      ${sharpJpeg.ms.toFixed(1)}ms, ${sharpJpeg.result.length} bytes`);

    const diff = rustJpeg.result.length - sharpJpeg.result.length;
    const pct = ((diff / sharpJpeg.result.length) * 100).toFixed(1);
    console.log(`  → Size diff: ${diff} bytes (${pct}%) ${diff < 0 ? '✅ Rust smaller' : '⚠️  Sharp smaller'}`);
    
    const timeDiff = rustJpeg.ms - sharpJpeg.ms;
    console.log(`  → Time diff: ${timeDiff.toFixed(1)}ms ${timeDiff < 0 ? '✅ Rust faster' : ''}`);
  }

  console.log('');

  // ============================================================================
  // TEST 2: WebP encoding
  // ============================================================================
  console.log('📸 Test 2: WebP encoding (quality=80)');
  console.log('-'.repeat(40));

  const rustWebp = await benchmark('lazy-image', async () => {
    return ImageEngine.from(inputBuf)
      .resize(800, null)
      .toBuffer('webp', 80);
  });

  fs.writeFileSync(path.join(outputDir, 'out_rust.webp'), rustWebp.result);
  console.log(`  lazy-image: ${rustWebp.ms.toFixed(1)}ms, ${rustWebp.result.length} bytes`);

  if (sharp) {
    const sharpWebp = await benchmark('sharp', async () => {
      return sharp(inputBuf)
        .resize(800)
        .webp({ quality: 80 })
        .toBuffer();
    });

    fs.writeFileSync(path.join(outputDir, 'out_sharp.webp'), sharpWebp.result);
    console.log(`  sharp:      ${sharpWebp.ms.toFixed(1)}ms, ${sharpWebp.result.length} bytes`);
  }

  console.log('');

  // ============================================================================
  // TEST 3: Pipeline with multiple operations
  // ============================================================================
  console.log('📸 Test 3: Complex pipeline (resize + rotate + grayscale)');
  console.log('-'.repeat(40));

  const rustComplex = await benchmark('lazy-image', async () => {
    return ImageEngine.from(inputBuf)
      .resize(600, null)
      .rotate(90)
      .grayscale()
      .toBuffer('jpeg', 80);
  });

  fs.writeFileSync(path.join(outputDir, 'out_complex.jpg'), rustComplex.result);
  console.log(`  lazy-image: ${rustComplex.ms.toFixed(1)}ms, ${rustComplex.result.length} bytes`);

  if (sharp) {
    const sharpComplex = await benchmark('sharp', async () => {
      return sharp(inputBuf)
        .resize(600)
        .rotate(90)
        .grayscale()
        .jpeg({ quality: 80, mozjpeg: true })
        .toBuffer();
    });

    fs.writeFileSync(path.join(outputDir, 'out_complex_sharp.jpg'), sharpComplex.result);
    console.log(`  sharp:      ${sharpComplex.ms.toFixed(1)}ms, ${sharpComplex.result.length} bytes`);
  }

  console.log('');

  // ============================================================================
  // TEST 4: Clone for multi-output
  // ============================================================================
  console.log('📸 Test 4: Multi-output (clone for JPEG + WebP)');
  console.log('-'.repeat(40));

  const engine = ImageEngine.from(inputBuf).resize(500, null);
  
  // Clone for different outputs
  const clone1 = engine.clone();
  const clone2 = engine.clone();

  const [jpegOut, webpOut] = await Promise.all([
    clone1.toBuffer('jpeg', 85),
    clone2.toBuffer('webp', 85),
  ]);

  console.log(`  JPEG: ${jpegOut.length} bytes`);
  console.log(`  WebP: ${webpOut.length} bytes`);

  console.log('');
  console.log('='.repeat(60));
  console.log('✅ All tests completed');
  console.log('='.repeat(60));
}

main().catch((err) => {
  console.error('❌ Error:', err);
  process.exit(1);
});
