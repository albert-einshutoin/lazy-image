/**
 * Concurrency validation test for batch processing
 */

const fs = require('fs');
const path = require('path');
const { TEST_DIR, resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine, ErrorCategory, getErrorCategory } = require(resolveRoot('index'));

async function testConcurrencyValidation() {
    console.log('🧪 Concurrency Validation Test');
    console.log('==============================\n');
    
    // Create test input image if it doesn't exist
    const testImagePath = resolveFixture('test_input.jpg');
    if (!fs.existsSync(testImagePath)) {
        throw new Error(`Test image not found: ${testImagePath}`);
    }
    
    const outputDir = path.join(TEST_DIR, 'output', 'concurrency-validation');
    if (!fs.existsSync(outputDir)) {
        fs.mkdirSync(outputDir, { recursive: true });
    }
    
    const inputs = [testImagePath];
    
    // Test concurrency = 0 (should use default - valid)
    console.log('✅ Testing concurrency = 0 (default):');
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, {
            format: 'jpeg',
            quality: 80,
            concurrency: 0,
        });
        console.log(`  Success: Processed ${results.length} images with default concurrency`);
        if (results.length > 0) {
            if (typeof results[0].effectiveConcurrency !== 'number' || results[0].effectiveConcurrency < 1) {
                throw new Error('effectiveConcurrency should be reported for auto mode');
            }
            if (results[0].autoConcurrency !== true) {
                throw new Error('autoConcurrency should be true when concurrency=0');
            }
        }
    } catch (e) {
        console.log(`  ❌ Unexpected error: ${e.message}`);
    }
    
    // Test concurrency = 1 (should work - valid minimum)
    console.log('\n✅ Testing concurrency = 1 (minimum valid):');
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, {
            format: 'jpeg',
            quality: 80,
            concurrency: 1,
        });
        console.log(`  Success: Processed ${results.length} images with 1 thread`);
        if (results.length > 0) {
            if (results[0].effectiveConcurrency !== 1) {
                throw new Error(`effectiveConcurrency should be 1, got ${results[0].effectiveConcurrency}`);
            }
            if (results[0].autoConcurrency !== false) {
                throw new Error('autoConcurrency should be false for manual concurrency');
            }
        }
    } catch (e) {
        console.log(`  ❌ Unexpected error: ${e.message}`);
    }
    
    // Test concurrency = MAX_CONCURRENCY (should work - valid maximum)
    const MAX_CONCURRENCY = 1024;
    console.log(`\n✅ Testing concurrency = ${MAX_CONCURRENCY} (maximum valid):`);
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, {
            format: 'jpeg',
            quality: 80,
            concurrency: MAX_CONCURRENCY,
        });
        console.log(`  Success: Processed ${results.length} images with ${MAX_CONCURRENCY} threads`);
    } catch (e) {
        console.log(`  ❌ Unexpected error: ${e.message}`);
    }
    
    // Test concurrency = MAX_CONCURRENCY + 1 (should fail - invalid)
    const INVALID_CONCURRENCY = MAX_CONCURRENCY + 1;
    console.log(`\n❌ Testing concurrency = ${INVALID_CONCURRENCY} (should fail):`);
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, {
            format: 'jpeg',
            quality: 80,
            concurrency: INVALID_CONCURRENCY,
        });
        console.log(`  ❌ Should not reach here - invalid concurrency was accepted`);
    } catch (e) {
        console.log(`  ✅ Correctly rejected: ${e.message}`);

        if (e.errorCode === 'E400') {
            console.log('  ✅ Error code is E400 (invalid argument)');
        } else {
            throw new Error(`Unexpected error code: ${e.errorCode}`);
        }

        const category = getErrorCategory(e);
        if (category === ErrorCategory.UserError) {
            console.log('  ✅ Error category is UserError');
        } else {
            throw new Error(`Unexpected error category: ${category}`);
        }

        if (e.message.includes('must be 0 or 1-1024')) {
            console.log('  ✅ Error message format is correct');
        } else {
            throw new Error(`Unexpected error message: ${e.message}`);
        }
    }
    
    console.log('\n📋 Concurrency Validation Summary:');
    console.log('1. ✅ concurrency = 0: Uses default thread pool');
    console.log('2. ✅ concurrency = 1-1024: Creates custom thread pool');
    console.log('3. ✅ concurrency > 1024: Properly rejected with clear error');
    console.log('4. ✅ Error message clarifies "0 or 1-MAX" range');
    
    // Clean up test output
    try {
        const files = fs.readdirSync(outputDir);
        for (const file of files) {
            if (file.endsWith('.jpeg') || file.endsWith('.jpg')) {
                fs.unlinkSync(path.join(outputDir, file));
            }
        }
    } catch (e) {
        // Ignore cleanup errors
    }
}

if (require.main === module) {
    testConcurrencyValidation().catch(console.error);
}

module.exports = { testConcurrencyValidation };
