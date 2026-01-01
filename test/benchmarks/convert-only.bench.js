/**
 * Benchmark: Format Conversion Only (No Resize)
 * 
 * This benchmark tests pure format conversion without pixel manipulation.
 * lazy-image's Copy-on-Write (CoW) architecture should excel here:
 * - No intermediate buffer allocations
 * - Direct decode → encode pipeline
 * - Memory efficiency advantage
 */

const fs = require('fs');
const path = require('path');
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths');
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
const OUTPUT_DIR = resolveTemp('benchmarks', 'convert-only');

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

// Helper to format memory
function formatMemory(bytes) {
    return (bytes / 1024 / 1024).toFixed(2) + ' MB';
}

async function benchmarkLazyImageConvert(format, quality) {
    // Force garbage collection if available
    if (global.gc) global.gc();
    const memBefore = process.memoryUsage().heapUsed;
    
    const start = Date.now();
    // No resize - pure format conversion
    const buffer = await ImageEngine.fromPath(TEST_IMAGE)
        .toBuffer(format, quality);
    const time = Date.now() - start;
    
    const memAfter = process.memoryUsage().heapUsed;
    const memDelta = Math.max(0, memAfter - memBefore);
    
    return { size: buffer.length, time, memDelta };
}

async function benchmarkSharpConvert(format, quality) {
    // Force garbage collection if available
    if (global.gc) global.gc();
    const memBefore = process.memoryUsage().heapUsed;
    
    const start = Date.now();
    let sharpInstance = sharp(TEST_IMAGE);
    
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
    const memAfter = process.memoryUsage().heapUsed;
    const memDelta = Math.max(0, memAfter - memBefore);
    
    return { size: buffer.length, time, memDelta };
}

async function runBenchmark() {
    console.log('=== Format Conversion Benchmark (No Resize) ===\n');
    console.log('This test measures pure format conversion performance.');
    console.log('lazy-image\'s CoW architecture should excel here.\n');
    console.log(`Input: ${TEST_IMAGE}`);
    const stats = fs.statSync(TEST_IMAGE);
    console.log(`Size: ${(stats.size / 1024 / 1024).toFixed(1)} MB\n`);
    console.log('Conditions: NO resize, format conversion only\n');
    console.log('─'.repeat(70));
    
    const results = [];
    
    // Test formats
    const testCases = [
        { format: 'webp', quality: 80, name: 'PNG → WebP' },
        { format: 'avif', quality: 60, name: 'PNG → AVIF' },
        { format: 'jpeg', quality: 80, name: 'PNG → JPEG' },
    ];
    
    for (const testCase of testCases) {
        const { format, quality, name } = testCase;
        console.log(`\n📊 Testing ${name} (quality ${quality})...`);
        
        try {
            // lazy-image
            console.log('  Testing lazy-image...');
            const lazyResult = await benchmarkLazyImageConvert(format, quality);
            
            // sharp
            console.log('  Testing sharp...');
            const sharpResult = await benchmarkSharpConvert(format, quality);
            
            results.push({
                format: name,
                quality,
                lazy: lazyResult,
                sharp: sharpResult,
            });
            
            console.log(`  ✅ lazy-image: ${formatBytes(lazyResult.size)} bytes (${lazyResult.time}ms)`);
            console.log(`  ✅ sharp:      ${formatBytes(sharpResult.size)} bytes (${sharpResult.time}ms)`);
            
            const speedRatio = (sharpResult.time / lazyResult.time).toFixed(2);
            const speedEmoji = lazyResult.time < sharpResult.time ? '⚡' : '🐢';
            console.log(`  ${speedEmoji} Speed: ${speedRatio}x ${lazyResult.time < sharpResult.time ? 'faster' : 'slower'}`);
            
        } catch (e) {
            console.error(`  ❌ Error testing ${name}:`, e.message);
        }
    }
    
    // Summary table - Speed
    console.log('\n' + '─'.repeat(70));
    console.log('\n📊 Format Conversion Speed (No Resize)\n');
    console.log('| Conversion | lazy-image | sharp | Speed Ratio |');
    console.log('|------------|------------|-------|-------------|');
    
    for (const result of results) {
        if (result.sharp) {
            const speedRatio = (result.sharp.time / result.lazy.time).toFixed(2);
            const emoji = result.lazy.time < result.sharp.time ? '⚡' : '🐢';
            console.log(`| **${result.format}** | ${result.lazy.time}ms | ${result.sharp.time}ms | ${emoji} ${speedRatio}x |`);
        }
    }
    
    // Summary table - File Size
    console.log('\n📊 Output File Size\n');
    console.log('| Conversion | lazy-image | sharp | Difference |');
    console.log('|------------|------------|-------|------------|');
    
    for (const result of results) {
        if (result.sharp) {
            const lazySize = formatBytes(result.lazy.size);
            const sharpSize = formatBytes(result.sharp.size);
            const diff = calcDiff(result.lazy.size, result.sharp.size);
            const emoji = result.lazy.size <= result.sharp.size ? '✅' : '⚠️';
            console.log(`| **${result.format}** | ${lazySize} bytes | ${sharpSize} bytes | ${emoji} ${diff} |`);
        }
    }
    
    console.log('\n' + '─'.repeat(70));
    console.log('\n✅ Format conversion benchmark completed!\n');
    
    return results;
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

