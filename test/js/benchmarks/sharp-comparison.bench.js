/**
 * Benchmark comparison: lazy-image vs sharp
 * Based on README.md benchmark conditions:
 * - 23MB PNG input
 * - Resize to 800px (width, auto height)
 * - Quality 60-80
 */

const fs = require('fs');
const path = require('path');
const { resolveFixture, resolveRoot, TEST_DIR } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));

// Check if sharp is available
let sharp;
try {
    sharp = require('sharp');
} catch (e) {
    console.error('❌ sharp is not installed. Please install it first:');
    console.error('   npm install sharp');
    process.exit(1);
}

const TEST_IMAGE = resolveFixture('test_input.png');
const OUTPUT_DIR = path.join(TEST_DIR, 'output', 'benchmarks', 'sharp-comparison');

// Ensure output directory exists
if (!fs.existsSync(OUTPUT_DIR)) {
    fs.mkdirSync(OUTPUT_DIR, { recursive: true });
}

// Helper to format bytes
function formatBytes(bytes) {
    return bytes.toLocaleString('en-US', { maximumFractionDigits: 0 });
}

// Helper to calculate percentage difference
function calcDiff(lazy, sharp) {
    const diff = ((lazy - sharp) / sharp) * 100;
    return diff > 0 ? `+${diff.toFixed(1)}%` : `${diff.toFixed(1)}%`;
}

async function benchmarkLazyImage(format, quality) {
    const start = Date.now();
    const buffer = await ImageEngine.fromPath(TEST_IMAGE)
        .resize(800, null)
        .toBuffer(format, quality);
    const time = Date.now() - start;
    return { size: buffer.length, time };
}

async function benchmarkSharp(format, quality) {
    const start = Date.now();
    let sharpInstance = sharp(TEST_IMAGE)
        .resize(800, null, { withoutEnlargement: true });
    
    let buffer;
    if (format === 'jpeg') {
        buffer = await sharpInstance.jpeg({ quality, mozjpeg: true }).toBuffer();
    } else if (format === 'webp') {
        buffer = await sharpInstance.webp({ quality }).toBuffer();
    } else if (format === 'avif') {
        buffer = await sharpInstance.avif({ quality }).toBuffer();
    } else {
        throw new Error(`Unsupported format: ${format}`);
    }
    
    const time = Date.now() - start;
    return { size: buffer.length, time };
}

async function runBenchmark() {
    console.log('=== lazy-image vs sharp Benchmark ===\n');
    console.log(`Input: ${TEST_IMAGE}`);
    const stats = fs.statSync(TEST_IMAGE);
    console.log(`Size: ${(stats.size / 1024 / 1024).toFixed(1)} MB\n`);
    console.log('Conditions: resize to 800px width, auto height\n');
    console.log('─'.repeat(60));
    
    const results = [];
    
    // Test formats
    const testCases = [
        { format: 'jpeg', quality: 80, name: 'JPEG' },
        { format: 'webp', quality: 80, name: 'WebP' },
        { format: 'avif', quality: 60, name: 'AVIF' },
    ];
    
    for (const testCase of testCases) {
        const { format, quality, name } = testCase;
        console.log(`\n📊 Testing ${name} (quality ${quality})...`);
        
        try {
            // lazy-image
            console.log('  Testing lazy-image...');
            const lazyResult = await benchmarkLazyImage(format, quality);
            
            // sharp
            console.log('  Testing sharp...');
            let sharpResult;
            if (format === 'avif') {
                try {
                    sharpResult = await benchmarkSharp(format, quality);
                } catch (e) {
                    if (e.message.includes('avif') || e.message.includes('AVIF')) {
                        console.log('  ⚠️  sharp does not support AVIF (or not available)');
                        sharpResult = null;
                    } else {
                        throw e;
                    }
                }
            } else {
                sharpResult = await benchmarkSharp(format, quality);
            }
            
            results.push({
                format: name,
                quality,
                lazy: lazyResult,
                sharp: sharpResult,
            });
            
            console.log(`  ✅ lazy-image: ${formatBytes(lazyResult.size)} bytes (${lazyResult.time}ms)`);
            if (sharpResult) {
                console.log(`  ✅ sharp:      ${formatBytes(sharpResult.size)} bytes (${sharpResult.time}ms)`);
                const sizeDiff = calcDiff(lazyResult.size, sharpResult.size);
                const sizeEmoji = lazyResult.size < sharpResult.size ? '✅' : '⚠️';
                const speedRatio = (sharpResult.time / lazyResult.time).toFixed(2);
                const speedEmoji = lazyResult.time < sharpResult.time ? '⚡' : '🐢';
                console.log(`  ${sizeEmoji} Size diff: ${sizeDiff}`);
                console.log(`  ${speedEmoji} Speed: ${speedRatio}x ${lazyResult.time < sharpResult.time ? 'faster' : 'slower'}`);
            }
        } catch (e) {
            console.error(`  ❌ Error testing ${name}:`, e.message);
        }
    }
    
    // Complex pipeline test (resize + rotate + grayscale)
    console.log('\n📊 Testing Complex Pipeline (resize + rotate + grayscale)...');
    try {
        // lazy-image
        console.log('  Testing lazy-image...');
        const lazyComplexStart = Date.now();
        const lazyComplex = await ImageEngine.fromPath(TEST_IMAGE)
            .resize(800, null)
            .rotate(90)
            .grayscale()
            .toBuffer('jpeg', 75);
        const lazyComplexTime = Date.now() - lazyComplexStart;
        
        // sharp
        console.log('  Testing sharp...');
        const sharpComplexStart = Date.now();
        const sharpComplex = await sharp(TEST_IMAGE)
            .resize(800, null, { withoutEnlargement: true })
            .rotate(90)
            .greyscale()
            .jpeg({ quality: 75, mozjpeg: true })
            .toBuffer();
        const sharpComplexTime = Date.now() - sharpComplexStart;
        
        results.push({
            format: 'Complex Pipeline',
            quality: 75,
            lazy: { size: lazyComplex.length, time: lazyComplexTime },
            sharp: { size: sharpComplex.length, time: sharpComplexTime },
        });
        
        console.log(`  ✅ lazy-image: ${formatBytes(lazyComplex.length)} bytes (${lazyComplexTime}ms)`);
        console.log(`  ✅ sharp:      ${formatBytes(sharpComplex.length)} bytes (${sharpComplexTime}ms)`);
        const sizeDiff = calcDiff(lazyComplex.length, sharpComplex.length);
        const sizeEmoji = lazyComplex.length < sharpComplex.length ? '✅' : '⚠️';
        const speedRatio = (sharpComplexTime / lazyComplexTime).toFixed(2);
        const speedEmoji = lazyComplexTime < sharpComplexTime ? '⚡' : '🐢';
        console.log(`  ${sizeEmoji} Size diff: ${sizeDiff}`);
        console.log(`  ${speedEmoji} Speed: ${speedRatio}x ${lazyComplexTime < sharpComplexTime ? 'faster' : 'slower'}`);
    } catch (e) {
        console.error(`  ❌ Error testing complex pipeline:`, e.message);
    }
    
    // Summary table - File Size
    console.log('\n' + '─'.repeat(80));
    console.log('\n📊 Summary Table - File Size\n');
    console.log('| Format | lazy-image | sharp | Difference |');
    console.log('|--------|------------|-------|------------|');
    
    for (const result of results) {
        const lazySize = formatBytes(result.lazy.size);
        if (result.sharp) {
            const sharpSize = formatBytes(result.sharp.size);
            const diff = calcDiff(result.lazy.size, result.sharp.size);
            const emoji = result.lazy.size < result.sharp.size ? '✅' : '⚠️';
            console.log(`| **${result.format}** | ${lazySize} bytes | ${sharpSize} bytes | ${emoji} ${diff} |`);
        } else {
            console.log(`| **${result.format}** | ${lazySize} bytes | N/A | 🏆 **Next-gen** |`);
        }
    }
    
    // Summary table - Speed
    console.log('\n📊 Summary Table - Processing Speed\n');
    console.log('| Format | lazy-image | sharp | Speed Ratio |');
    console.log('|--------|------------|-------|-------------|');
    
    for (const result of results) {
        if (result.sharp) {
            const speedRatio = (result.sharp.time / result.lazy.time).toFixed(2);
            const emoji = result.lazy.time < result.sharp.time ? '⚡' : '🐢';
            console.log(`| **${result.format}** | ${result.lazy.time}ms | ${result.sharp.time}ms | ${emoji} ${speedRatio}x |`);
        } else {
            console.log(`| **${result.format}** | ${result.lazy.time}ms | N/A | - |`);
        }
    }
    
    console.log('\n' + '─'.repeat(60));
    console.log('\n✅ Benchmark completed!\n');
}

// Cleanup function
function cleanup() {
    if (fs.existsSync(OUTPUT_DIR)) {
        fs.readdirSync(OUTPUT_DIR).forEach(file => {
            fs.unlinkSync(path.join(OUTPUT_DIR, file));
        });
        fs.rmdirSync(OUTPUT_DIR);
    }
}

// Run benchmark
runBenchmark()
    .then(() => {
        cleanup();
        process.exit(0);
    })
    .catch(e => {
        console.error('Benchmark error:', e);
        cleanup();
        process.exit(1);
    });
