/**
 * Benchmark verification: Compare actual results with README.md values
 * Tests multiple input files:
 * - test_4.5MB_5000x5000.png - large PNG for benchmarks (4.5MB, 5000×5000)
 * - test_100KB_1057x1057.jpg - medium JPEG for benchmarks (100KB, 1057×1057)
 * - test_95KB.avif - medium AVIF for benchmarks (95KB)
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

// Test input files - using new test fixtures
const TEST_FILES = [
    { path: resolveFixture('test_4.5MB_5000x5000.png'), name: 'test_4.5MB_5000x5000.png (4.5MB PNG, 5000×5000)' },
    { path: resolveFixture('test_100KB_1057x1057.jpg'), name: 'test_100KB_1057x1057.jpg (100KB JPEG, 1057×1057)' },
    { path: resolveFixture('test_95KB.avif'), name: 'test_95KB.avif (95KB AVIF)' },
];

// README.md expected values - disabled for new test fixtures
// Note: New test fixtures have different sizes, so expected values need to be updated
const README_VALUES = {
    // Values will be updated based on actual benchmark results with new fixtures
};

const OUTPUT_DIR = resolveTemp('benchmarks', 'readme-verification');

// Ensure output directory exists
if (!fs.existsSync(OUTPUT_DIR)) {
    fs.mkdirSync(OUTPUT_DIR, { recursive: true });
}

// Helper to format bytes
function formatBytes(bytes) {
    return bytes.toLocaleString('en-US', { maximumFractionDigits: 0 });
}

// Helper to calculate percentage difference
function calcDiff(actual, expected) {
    if (expected === null || expected === undefined) return null;
    const diff = ((actual - expected) / expected) * 100;
    return diff > 0 ? `+${diff.toFixed(1)}%` : `${diff.toFixed(1)}%`;
}

// Helper to check if values match (within tolerance)
function isWithinTolerance(actual, expected, tolerance = 0.05) {
    if (expected === null || expected === undefined) return null;
    const diff = Math.abs(actual - expected) / expected;
    return diff <= tolerance;
}

async function benchmarkLazyImage(inputPath, format, quality, operations = []) {
    const start = Date.now();
    let engine = ImageEngine.fromPath(inputPath);
    
    // Apply operations
    for (const op of operations) {
        if (op.type === 'resize') {
            engine = engine.resize(op.width, op.height);
        } else if (op.type === 'rotate') {
            engine = engine.rotate(op.angle);
        } else if (op.type === 'grayscale') {
            engine = engine.grayscale();
        }
    }
    
    const buffer = await engine.toBuffer(format, quality);
    const time = Date.now() - start;
    return { size: buffer.length, time };
}

async function benchmarkSharp(inputPath, format, quality, operations = []) {
    const start = Date.now();
    let sharpInstance = sharp(inputPath);
    
    // Apply operations
    for (const op of operations) {
        if (op.type === 'resize') {
            sharpInstance = sharpInstance.resize(op.width, op.height, { withoutEnlargement: true });
        } else if (op.type === 'rotate') {
            sharpInstance = sharpInstance.rotate(op.angle);
        } else if (op.type === 'grayscale') {
            sharpInstance = sharpInstance.greyscale();
        }
    }
    
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

async function runBenchmarkForFile(testFile) {
    const { path: inputPath, name: fileName } = testFile;
    
    // Check if file exists
    if (!fs.existsSync(inputPath)) {
        console.log(`\n⚠️  Skipping ${fileName}: File not found`);
        return null;
    }
    
    const stats = fs.statSync(inputPath);
    const fileSizeMB = (stats.size / 1024 / 1024).toFixed(2);
    const fileSizeKB = (stats.size / 1024).toFixed(1);
    
    console.log(`\n${'='.repeat(80)}`);
    console.log(`📁 Testing: ${fileName}`);
    console.log(`   Path: ${inputPath}`);
    console.log(`   Size: ${fileSizeMB} MB (${fileSizeKB} KB)`);
    console.log(`${'='.repeat(80)}`);
    
    const results = [];
    const readmeValues = README_VALUES[path.basename(inputPath)];
    
    // Test formats
    const testCases = [
        { format: 'jpeg', quality: 80, name: 'JPEG', operations: [{ type: 'resize', width: 800, height: null }] },
        { format: 'webp', quality: 80, name: 'WebP', operations: [{ type: 'resize', width: 800, height: null }] },
        { format: 'avif', quality: 60, name: 'AVIF', operations: [{ type: 'resize', width: 800, height: null }] },
    ];
    
    for (const testCase of testCases) {
        const { format, quality, name, operations } = testCase;
        console.log(`\n📊 Testing ${name} (quality ${quality})...`);
        
        try {
            // Check if input is AVIF (lazy-image doesn't support AVIF input)
            const inputExt = path.extname(inputPath).toLowerCase();
            if (inputExt === '.avif') {
                console.log('  ⚠️  Skipping: lazy-image does not support AVIF input (only output)');
                continue;
            }
            
            // lazy-image
            console.log('  Testing lazy-image...');
            const lazyResult = await benchmarkLazyImage(inputPath, format, quality, operations);
            
            // sharp
            console.log('  Testing sharp...');
            let sharpResult;
            try {
                sharpResult = await benchmarkSharp(inputPath, format, quality, operations);
            } catch (e) {
                if (e.message.includes('avif') || e.message.includes('AVIF')) {
                    console.log('  ⚠️  sharp does not support AVIF (or not available)');
                    sharpResult = null;
                } else {
                    throw e;
                }
            }
            
            // Compare with README values if available
            let readmeComparison = null;
            if (readmeValues && readmeValues[format]) {
                const expected = readmeValues[format];
                readmeComparison = {
                    sizeMatch: isWithinTolerance(lazyResult.size, expected.lazy, 0.01), // 1% tolerance
                    sizeDiff: calcDiff(lazyResult.size, expected.lazy),
                    timeMatch: isWithinTolerance(lazyResult.time, expected.lazyTime, 0.10), // 10% tolerance for time
                    timeDiff: calcDiff(lazyResult.time, expected.lazyTime),
                };
            }
            
            results.push({
                format: name,
                quality,
                lazy: lazyResult,
                sharp: sharpResult,
                readmeComparison,
            });
            
            console.log(`  ✅ lazy-image: ${formatBytes(lazyResult.size)} bytes (${lazyResult.time}ms)`);
            if (readmeComparison) {
                const sizeStatus = readmeComparison.sizeMatch ? '✅' : '⚠️';
                const timeStatus = readmeComparison.timeMatch ? '✅' : '⚠️';
                console.log(`     README: ${formatBytes(readmeValues[format].lazy)} bytes (${readmeValues[format].lazyTime}ms)`);
                console.log(`     ${sizeStatus} Size: ${readmeComparison.sizeDiff || 'N/A'}`);
                console.log(`     ${timeStatus} Time: ${readmeComparison.timeDiff || 'N/A'}`);
            }
            
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
        // Check if input is AVIF (lazy-image doesn't support AVIF input)
        const inputExt = path.extname(inputPath).toLowerCase();
        if (inputExt === '.avif') {
            console.log('  ⚠️  Skipping: lazy-image does not support AVIF input (only output)');
        } else {
            const operations = [
                { type: 'resize', width: 800, height: null },
                { type: 'rotate', angle: 90 },
                { type: 'grayscale' }
            ];
            
            // lazy-image
            console.log('  Testing lazy-image...');
            const lazyComplex = await benchmarkLazyImage(inputPath, 'jpeg', 75, operations);
        
            // sharp
            console.log('  Testing sharp...');
            const sharpComplex = await benchmarkSharp(inputPath, 'jpeg', 75, operations);
            
            // Compare with README values if available
            let readmeComparison = null;
            if (readmeValues && readmeValues.complex) {
                const expected = readmeValues.complex;
                readmeComparison = {
                    sizeMatch: isWithinTolerance(lazyComplex.size, expected.lazy, 0.01),
                    sizeDiff: calcDiff(lazyComplex.size, expected.lazy),
                    timeMatch: isWithinTolerance(lazyComplex.time, expected.lazyTime, 0.10),
                    timeDiff: calcDiff(lazyComplex.time, expected.lazyTime),
                };
            }
            
            results.push({
                format: 'Complex Pipeline',
                quality: 75,
                lazy: lazyComplex,
                sharp: sharpComplex,
                readmeComparison,
            });
            
            console.log(`  ✅ lazy-image: ${formatBytes(lazyComplex.size)} bytes (${lazyComplex.time}ms)`);
            if (readmeComparison) {
                const sizeStatus = readmeComparison.sizeMatch ? '✅' : '⚠️';
                const timeStatus = readmeComparison.timeMatch ? '✅' : '⚠️';
                console.log(`     README: ${formatBytes(readmeValues.complex.lazy)} bytes (${readmeValues.complex.lazyTime}ms)`);
                console.log(`     ${sizeStatus} Size: ${readmeComparison.sizeDiff || 'N/A'}`);
                console.log(`     ${timeStatus} Time: ${readmeComparison.timeDiff || 'N/A'}`);
            }
            console.log(`  ✅ sharp:      ${formatBytes(sharpComplex.size)} bytes (${sharpComplex.time}ms)`);
            const sizeDiff = calcDiff(lazyComplex.size, sharpComplex.size);
            const sizeEmoji = lazyComplex.size < sharpComplex.size ? '✅' : '⚠️';
            const speedRatio = (sharpComplex.time / lazyComplex.time).toFixed(2);
            const speedEmoji = lazyComplex.time < sharpComplex.time ? '⚡' : '🐢';
            console.log(`  ${sizeEmoji} Size diff: ${sizeDiff}`);
            console.log(`  ${speedEmoji} Speed: ${speedRatio}x ${lazyComplex.time < sharpComplex.time ? 'faster' : 'slower'}`);
        }
    } catch (e) {
        console.error(`  ❌ Error testing complex pipeline:`, e.message);
    }
    
    return { fileName, results };
}

async function runAllBenchmarks() {
    console.log('='.repeat(80));
    console.log('🔬 Benchmark Verification: lazy-image vs sharp');
    console.log('   Comparing actual results with README.md values');
    console.log('='.repeat(80));
    
    const allResults = [];
    
    for (const testFile of TEST_FILES) {
        const result = await runBenchmarkForFile(testFile);
        if (result) {
            allResults.push(result);
        }
    }
    
    // Summary for test_4.5MB_5000x5000.png (README comparison)
    // Note: README values are disabled for new test fixtures
    // Uncomment and update README_VALUES when new baseline is established
    /*
    console.log('\n' + '='.repeat(80));
    console.log('📊 Summary: README.md Verification (test_4.5MB_5000x5000.png)');
    console.log('='.repeat(80));
    
    const pngResults = allResults.find(r => r.fileName.includes('test_50MB.png'));
    if (pngResults) {
        console.log('\n### File Size Comparison\n');
        console.log('| Format | lazy-image (Actual) | README Value | Match | Difference |');
        console.log('|--------|---------------------|--------------|-------|------------|');
        
        for (const result of pngResults.results) {
            if (result.readmeComparison) {
                const expected = README_VALUES['test_4.5MB_5000x5000.png']?.[result.format.toLowerCase().replace(' ', '')];
                if (expected) {
                    const match = result.readmeComparison.sizeMatch ? '✅' : '⚠️';
                    const diff = result.readmeComparison.sizeDiff || 'N/A';
                    console.log(`| **${result.format}** | ${formatBytes(result.lazy.size)} | ${formatBytes(expected.lazy)} | ${match} | ${diff} |`);
                }
            }
        }
        
        console.log('\n### Processing Speed Comparison\n');
        console.log('| Format | lazy-image (Actual) | README Value | Match | Difference |');
        console.log('|--------|---------------------|--------------|-------|------------|');
        
        for (const result of pngResults.results) {
            if (result.readmeComparison) {
                const expected = README_VALUES['test_4.5MB_5000x5000.png']?.[result.format.toLowerCase().replace(' ', '')];
                if (expected) {
                    const match = result.readmeComparison.timeMatch ? '✅' : '⚠️';
                    const diff = result.readmeComparison.timeDiff || 'N/A';
                    console.log(`| **${result.format}** | ${result.lazy.time}ms | ${expected.lazyTime}ms | ${match} | ${diff} |`);
                }
            }
        }
    }
    */
    
    // Summary for all files
    console.log('\n' + '='.repeat(80));
    console.log('📊 Summary: All Test Files');
    console.log('='.repeat(80));
    
    for (const fileResult of allResults) {
        console.log(`\n### ${fileResult.fileName}\n`);
        console.log('| Format | lazy-image | sharp | Size Diff | Speed Ratio |');
        console.log('|--------|------------|-------|-----------|-------------|');
        
        for (const result of fileResult.results) {
            if (result.sharp) {
                const sizeDiff = calcDiff(result.lazy.size, result.sharp.size);
                const speedRatio = (result.sharp.time / result.lazy.time).toFixed(2);
                const speedEmoji = result.lazy.time < result.sharp.time ? '⚡' : '🐢';
                console.log(`| **${result.format}** | ${formatBytes(result.lazy.size)} (${result.lazy.time}ms) | ${formatBytes(result.sharp.size)} (${result.sharp.time}ms) | ${sizeDiff} | ${speedEmoji} ${speedRatio}x |`);
            } else {
                console.log(`| **${result.format}** | ${formatBytes(result.lazy.size)} (${result.lazy.time}ms) | N/A | - | - |`);
            }
        }
    }
    
    console.log('\n' + '='.repeat(80));
    console.log('✅ Benchmark verification completed!\n');
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
runAllBenchmarks()
    .then(() => {
        cleanup();
        process.exit(0);
    })
    .catch(e => {
        console.error('Benchmark error:', e);
        cleanup();
        process.exit(1);
    });
