/**
 * Runtime verification of type-safety behavior.
 */

const { resolveFixture, resolveRoot } = require('../helpers/paths');
const { ImageEngine } = require(resolveRoot('index'));

async function testTypeSafetyInPractice() {
    console.log('🔒 Type Safety Verification Test');
    console.log('=================================\n');
    
    const imagePath = resolveFixture('test_input.jpg');
    
    // Valid formats test
    console.log('✅ Valid formats test:');
    const validFormats = ['jpeg', 'jpg', 'webp'];
    
    for (const format of validFormats) {
        try {
            const engine = ImageEngine.fromPath(imagePath);
            const fitMode = format === 'jpeg' ? 'cover' : format === 'jpg' ? 'fill' : undefined;
            const result = await engine.resize(200, 200, fitMode).toBuffer(format, 80);
            console.log(`  ${format}: ${result.length} bytes`);
        } catch (e) {
            console.log(`  ${format}: Error - ${e.message}`);
        }
    }
    
    // Preset test
    console.log('\n✅ Valid presets test:');
    const validPresets = ['thumbnail', 'avatar', 'hero', 'social'];
    
    for (const presetName of validPresets) {
        try {
            const engine = ImageEngine.fromPath(imagePath);
            const preset = engine.preset(presetName);
            
            console.log(`  ${presetName}:`);
            console.log(`    Format: ${preset.format}`);
            console.log(`    Quality: ${preset.quality}`);
            console.log(`    Size: ${preset.width}x${preset.height}`);
            
            // Type-safe in TypeScript; runtime checks in JavaScript
            const buffer = await engine.toBuffer(preset.format, preset.quality);
            console.log(`    Result: ${buffer.length} bytes`);

            const convenience = await ImageEngine.fromPath(imagePath).toBufferWithPreset(presetName);
            console.log(`    Convenience result: ${convenience.length} bytes`);
        } catch (e) {
            console.log(`  ${presetName}: Error - ${e.message}`);
        }
    }
    
    // Invalid values test (should fail at runtime)
    console.log('\n❌ Invalid values test (should fail at runtime):');
    
    try {
        const engine = ImageEngine.fromPath(imagePath);
        await engine.toBuffer('invalid_format', 80);
        console.log('  ❌ Should not reach here - invalid format was accepted');
    } catch (e) {
        console.log('  ✅ Invalid format correctly rejected:', e.message.substring(0, 100) + '...');
    }
    
    try {
        const engine = ImageEngine.fromPath(imagePath);
        engine.preset('invalid_preset');
        console.log('  ❌ Should not reach here - invalid preset was accepted');
    } catch (e) {
        console.log('  ✅ Invalid preset correctly rejected:', e.message.substring(0, 100) + '...');
    }
    
    console.log('\n📋 Type Safety Benefits:');
    console.log('1. ✅ IDE autocomplete for format strings');
    console.log('2. ✅ Compile-time validation in TypeScript');
    console.log('3. ✅ Runtime validation in both TypeScript and JavaScript');
    console.log('4. ✅ Better developer experience and fewer bugs');
    
    console.log('\n🎯 Before vs After:');
    console.log('Before: toBuffer(format: string) - accepts ANY string');
    console.log('After:  toBuffer(format: OutputFormat) - only valid formats');
    console.log('Result: Compile-time safety + IDE support');
}

if (require.main === module) {
    testTypeSafetyInPractice().catch(console.error);
}
