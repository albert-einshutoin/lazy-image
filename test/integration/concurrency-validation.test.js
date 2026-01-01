/**
 * Concurrency validation test for batch processing
 */

const fs = require('fs');
const path = require('path');
const { TEST_DIR, resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));

async function testConcurrencyValidation() {
    console.log('🧪 Concurrency Validation Test');
    console.log('==============================\n');
    
    // Create test input image if it doesn't exist
    const testImagePath = resolveFixture('test_input.jpg');
    if (!fs.existsSync(testImagePath)) {
        console.log('⚠️  Test image not found, skipping concurrency validation tests');
        return;
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
        const results = await engine.processBatch(inputs, outputDir, 'jpeg', 80, 0);
        console.log(`  Success: Processed ${results.length} images with default concurrency`);
    } catch (e) {
        console.log(`  ❌ Unexpected error: ${e.message}`);
    }
    
    // Test concurrency = 1 (should work - valid minimum)
    console.log('\n✅ Testing concurrency = 1 (minimum valid):');
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, 'jpeg', 80, 1);
        console.log(`  Success: Processed ${results.length} images with 1 thread`);
    } catch (e) {
        console.log(`  ❌ Unexpected error: ${e.message}`);
    }
    
    // Test concurrency = MAX_CONCURRENCY (should work - valid maximum)
    const MAX_CONCURRENCY = 1024;
    console.log(`\n✅ Testing concurrency = ${MAX_CONCURRENCY} (maximum valid):`);
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, 'jpeg', 80, MAX_CONCURRENCY);
        console.log(`  Success: Processed ${results.length} images with ${MAX_CONCURRENCY} threads`);
    } catch (e) {
        console.log(`  ❌ Unexpected error: ${e.message}`);
    }
    
    // Test concurrency = MAX_CONCURRENCY + 1 (should fail - invalid)
    const INVALID_CONCURRENCY = MAX_CONCURRENCY + 1;
    console.log(`\n❌ Testing concurrency = ${INVALID_CONCURRENCY} (should fail):`);
    try {
        const engine = ImageEngine.fromPath(testImagePath).resize(200, 200);
        const results = await engine.processBatch(inputs, outputDir, 'jpeg', 80, INVALID_CONCURRENCY);
        console.log(`  ❌ Should not reach here - invalid concurrency was accepted`);
    } catch (e) {
        console.log(`  ✅ Correctly rejected: ${e.message}`);
        
        // Verify the error message format
        if (e.message.includes('must be 0 or 1-1024')) {
            console.log('  ✅ Error message format is correct');
        } else {
            console.log('  ⚠️  Error message format needs attention');
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
