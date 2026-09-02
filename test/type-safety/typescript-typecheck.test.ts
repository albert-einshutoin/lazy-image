/**
 * TypeScript type-safety test file.
 * Verifies that type definitions work correctly.
 */
import * as path from 'path';
import {
    ArtifactPlan,
    ArtifactSourceFacts,
    ArtifactCompilationError,
    BatchProgressEvent,
    CompileImageOptions,
    compileArtifactPlan,
    compileImage,
    ImageEngine,
    ImageArtifactManifest,
    ImageMetadata,
    OutputFormat,
    PublicUploadPolicy,
    PresetName,
    PresetResult,
    ResponsiveFilesOutput,
    ResponsiveSetOptions,
    ResponsiveVariant,
    ResizeFit,
    processBatchChunked,
} from '../../index';
import { createStreamingPipeline } from '../../streaming/pipeline';

async function testTypeSafety() {
    const imagePath = path.resolve(__dirname, '../fixtures/test_input.jpg');

    const artifactSource: ArtifactSourceFacts = {
        sha256: 'a'.repeat(64),
        bytes: 1024,
        format: 'jpeg',
        width: 1280,
        height: 720,
        hasAlpha: false,
        isAnimated: false,
        orientationKnown: true,
        orientation: null,
    };
    const publicUploadPolicy: PublicUploadPolicy = {
        preset: 'publicUpload',
        widths: [320, 640],
        formats: ['webp'],
        placeholder: true,
    };
    const artifactPlan = null as ArtifactPlan | null;
    const compiledArtifactPlan: ArtifactPlan = compileArtifactPlan(
        artifactSource,
        publicUploadPolicy,
        'b'.repeat(64),
    );
    console.log(`Artifact policy: ${artifactSource.format} ${publicUploadPolicy.widths?.length} ${artifactPlan} ${compiledArtifactPlan.cacheKey}`);

    const compileOptions: CompileImageOptions = {
        inputPath: imagePath,
        outputDir: path.resolve(__dirname, '../../.tmp/type-safety-artifacts'),
        policy: publicUploadPolicy,
    };
    const compiledManifest: Promise<ImageArtifactManifest> = compileImage(compileOptions);
    const compilationError: ArtifactCompilationError | null = null;
    console.log(`Transactional compiler: ${compiledManifest} ${compilationError}`);
    
    // Type-safe OutputFormat usage
    const validFormats: OutputFormat[] = ['jpeg', 'jpg', 'png', 'webp', 'avif'];
    
    const fitModes: ResizeFit[] = ['inside', 'cover', 'fill'];
    for (let index = 0; index < validFormats.length; index++) {
        const format = validFormats[index];
        console.log(`Testing format: ${format}`);
        
        // These are type-safe and do not cause compile errors
    const engine = ImageEngine.fromPath(imagePath);
    const asyncEngine: ImageEngine = await ImageEngine.fromPathAsync(imagePath);
    await asyncEngine.toBuffer('jpeg', 80);
        const fitMode = fitModes[index % fitModes.length];
        const result = await engine.resize(400, 300, fitMode).toBuffer(format, 80);
        console.log(`✅ ${format}: ${result.length} bytes`);
    }

    // Explicit ResizeFit type usage
    const coverFit: ResizeFit = 'cover';
    await ImageEngine.fromPath(imagePath).resize(300, 300, coverFit).toBuffer('jpeg', 75);
    await ImageEngine.fromPath(imagePath).resize({ width: 320, fit: coverFit }).toBuffer('jpeg', 75);

    const responsiveOptions: ResponsiveSetOptions = {
        widths: [320, 640],
        format: 'webp',
        quality: 80,
    };
    const responsiveVariants: ResponsiveVariant[] = await ImageEngine.fromPath(imagePath)
        .toResponsiveSet(responsiveOptions);
    const responsiveFiles: ResponsiveFilesOutput = await ImageEngine.fromPath(imagePath)
        .toFilesResponsive(path.resolve(__dirname, '../../.tmp/image-{width}.webp'), responsiveOptions);
    console.log(`Responsive outputs: ${responsiveVariants.length} ${responsiveFiles.files.length}`);

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
    const alpha: boolean = metadata.hasAlpha;
    const animated: boolean = metadata.isAnimated;
    const orientation: number | undefined = metadata.orientation;
    console.log(`Traits: alpha=${alpha}, animated=${animated}, orientation=${orientation}`);
    
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

    const chunkedBatchResults = await processBatchChunked(
        [imagePath],
        path.resolve(__dirname, '../../.tmp/type-safety-batch-chunked'),
        {
            format: 'webp',
            quality: 80,
            chunkSize: 1,
            onProgress: (event: BatchProgressEvent) => {
                console.log(`Chunked progress: ${event.completed}/${event.total}`);
            },
        }
    );
    console.log(`Chunked batch processing: ${chunkedBatchResults.length} results`);

    const streamingPipeline = createStreamingPipeline({
        format: 'webp',
        ops: [{ op: 'resize', width: 128, fit: 'inside' }],
    });
    streamingPipeline.readable.resume();
    
    // @ts-expect-error invalid format must fail at compile time
    await ImageEngine.fromPath(imagePath).toBuffer('invalid_format', 80);

    // @ts-expect-error invalid preset must fail at compile time
    ImageEngine.fromPath(imagePath).preset('invalid_preset');
    
    console.log('✅ All type safety tests passed!');
}

// IDE autocomplete test helper
function testIDECompletion() {
    const engine = ImageEngine.fromPath('test.jpg');
    
    // These should trigger IDE autocomplete (use valid values in real usage)
    engine.toBuffer('jpeg', 80); // Autocomplete: 'jpeg', 'png', 'webp', 'avif'
    engine.preset('thumbnail'); // Autocomplete: 'thumbnail', 'avatar', 'hero', 'social'
    
    // Type inference test: preset.format inferred as canonical output format, preset.quality as number | undefined
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
