/**
 * TypeScript type-safety test file.
 * Verifies that type definitions work correctly.
 */
import * as path from 'path';
import { ImageEngine, OutputFormat, InputFormat, PresetName, ImageMetadata, PresetResult, ResizeFit } from '../../index';

async function testTypeSafety() {
    const imagePath = path.resolve(__dirname, '../fixtures/test_input.jpg');
    
    // Type-safe OutputFormat usage
    const validFormats: OutputFormat[] = ['jpeg', 'jpg', 'png', 'webp', 'avif'];
    
    const fitModes: ResizeFit[] = ['inside', 'cover', 'fill'];
    for (let index = 0; index < validFormats.length; index++) {
        const format = validFormats[index];
        console.log(`Testing format: ${format}`);
        
        // These are type-safe and do not cause compile errors
        const engine = ImageEngine.fromPath(imagePath);
        const fitMode = fitModes[index % fitModes.length];
        const result = await engine.resize(400, 300, fitMode).toBuffer(format, 80);
        console.log(`✅ ${format}: ${result.length} bytes`);
    }

    // Explicit ResizeFit type usage
    const coverFit: ResizeFit = 'cover';
    await ImageEngine.fromPath(imagePath).resize(300, 300, coverFit).toBuffer('jpeg', 75);

    // Uppercase format is also accepted
    await ImageEngine.fromPath(imagePath).toBuffer('JPEG', 80);
    
    // Presets are also type-safe
    const validPresets: PresetName[] = ['thumbnail', 'avatar', 'hero', 'social'];
    
    for (const presetName of validPresets) {
        const engine = ImageEngine.fromPath(imagePath);
        const preset: PresetResult = engine.preset(presetName);
        
        console.log(`Preset ${presetName}:`);
        console.log(`  Format: ${preset.format}`);
        console.log(`  Quality: ${preset.quality}`);
        console.log(`  Size: ${preset.width}x${preset.height}`);
        
        // Use preset result
        const buffer = await engine.toBuffer(preset.format, preset.quality || undefined);
        console.log(`✅ ${presetName}: ${buffer.length} bytes`);

        const convenience = await ImageEngine.fromPath(imagePath).toBufferWithPreset(presetName);
        console.log(`✅ convenience ${presetName}: ${convenience.length} bytes`);
    }

    // Uppercase preset name is also accepted
    ImageEngine.fromPath(imagePath).preset('Avatar');
    
    // Metadata inspection is also type-safe
    const metadata: ImageMetadata = await import('../../index').then(m => m.inspectFile(imagePath));
    console.log(`Image: ${metadata.width}x${metadata.height}, format: ${metadata.format}`);
    
    // Batch processing is also type-safe
    const batchEngine = ImageEngine.fromPath(imagePath).resize(200, 200);
    const batchResults = await batchEngine.processBatch(
        [imagePath],
        path.resolve(__dirname, '../../.tmp/type-safety-batch'),
        {
            format: 'jpeg',
            quality: 85,
            concurrency: 2,
        }
    );
    
    console.log(`Batch processing: ${batchResults.length} results`);
    
    // The following should cause compile errors (commented out)
    /*
    // ❌ Invalid format
    await engine.toBuffer('invalid_format', 80);
    
    // ❌ Invalid preset
    const invalidPreset = engine.preset('invalid_preset');
    
    // ❌ Type mismatch (OutputFormat cannot be assigned to string in strict mode)
    const wrongType: string = preset.format;
    */
    
    console.log('✅ All type safety tests passed!');
}

// IDE autocomplete test helper
function testIDECompletion() {
    const engine = ImageEngine.fromPath('test.jpg');
    
    // These should trigger IDE autocomplete (use valid values in real usage)
    engine.toBuffer('jpeg', 80); // Autocomplete: 'jpeg', 'png', 'webp', 'avif'
    engine.preset('thumbnail'); // Autocomplete: 'thumbnail', 'avatar', 'hero', 'social'
    
    // Type inference test: preset.format inferred as OutputFormat, preset.quality as number | undefined
    const preset = engine.preset('thumbnail');
}

// Error handling test
async function testErrorHandling() {
    try {
        const engine = ImageEngine.fromPath('nonexistent.jpg');
        await engine.toBuffer('jpeg', 80);
    } catch (error) {
        console.log('Expected error:', error);
    }
}

if (require.main === module) {
    testTypeSafety().catch(console.error);
}
