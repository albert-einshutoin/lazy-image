/**
 * TypeScript型安全性テストファイル
 * 型定義が正しく動作するかを確認
 */
import * as path from 'path';
import { ImageEngine, OutputFormat, InputFormat, PresetName, ImageMetadata, PresetResult, ResizeFit } from '../../index';

async function testTypeSafety() {
    const imagePath = path.resolve(__dirname, '../fixtures/test_input.jpg');
    
    // 型安全なOutputFormat使用例
    const validFormats: OutputFormat[] = ['jpeg', 'jpg', 'png', 'webp', 'avif'];
    
    const fitModes: ResizeFit[] = ['inside', 'cover', 'fill'];
    for (let index = 0; index < validFormats.length; index++) {
        const format = validFormats[index];
        console.log(`Testing format: ${format}`);
        
        // これらは型安全でコンパイルエラーにならない
        const engine = ImageEngine.fromPath(imagePath);
        const fitMode = fitModes[index % fitModes.length];
        const result = await engine.resize(400, 300, fitMode).toBuffer(format, 80);
        console.log(`✅ ${format}: ${result.length} bytes`);
    }

    // 明示的なResizeFit型の利用例
    const coverFit: ResizeFit = 'cover';
    await ImageEngine.fromPath(imagePath).resize(300, 300, coverFit).toBuffer('jpeg', 75);

    // 大文字フォーマットも許容
    await ImageEngine.fromPath(imagePath).toBuffer('JPEG', 80);
    
    // プリセットも型安全
    const validPresets: PresetName[] = ['thumbnail', 'avatar', 'hero', 'social'];
    
    for (const presetName of validPresets) {
        const engine = ImageEngine.fromPath(imagePath);
        const preset: PresetResult = engine.preset(presetName);
        
        console.log(`Preset ${presetName}:`);
        console.log(`  Format: ${preset.format}`); // OutputFormat型
        console.log(`  Quality: ${preset.quality}`);
        console.log(`  Size: ${preset.width}x${preset.height}`);
        
        // プリセット結果を使用
        const buffer = await engine.toBuffer(preset.format, preset.quality || undefined);
        console.log(`✅ ${presetName}: ${buffer.length} bytes`);
    }

    // 大文字プリセットも許容
    ImageEngine.fromPath(imagePath).preset('Avatar');
    
    // メタデータ取得も型安全
    const metadata: ImageMetadata = await import('../../index').then(m => m.inspectFile(imagePath));
    console.log(`Image: ${metadata.width}x${metadata.height}, format: ${metadata.format}`);
    
    // バッチ処理も型安全
    const batchEngine = ImageEngine.fromPath(imagePath).resize(200, 200);
    const batchResults = await batchEngine.processBatch(
        [imagePath],
        path.resolve(__dirname, '../../.tmp/type-safety-batch'),
        'jpeg', // OutputFormat型として扱われる
        85,
        undefined, // fastMode (optional)
        2
    );
    
    console.log(`Batch processing: ${batchResults.length} results`);
    
    // 以下はコンパイルエラーになるべき例（コメントアウト）
    /*
    // ❌ 無効なフォーマット
    await engine.toBuffer('invalid_format', 80);
    
    // ❌ 無効なプリセット
    const invalidPreset = engine.preset('invalid_preset');
    
    // ❌ 型の不一致
    const wrongType: string = preset.format; // OutputFormat型をstring型に代入はできない（strictモード）
    */
    
    console.log('✅ All type safety tests passed!');
}

// IDE補完テスト用関数
function testIDECompletion() {
    const engine = ImageEngine.fromPath('test.jpg');
    
    // これらでIDE補完が効くはず（実際の実装では有効な値を使用）
    engine.toBuffer('jpeg', 80); // 'jpeg', 'png', 'webp', 'avif'が補完候補に出るはず
    engine.preset('thumbnail'); // 'thumbnail', 'avatar', 'hero', 'social'が補完候補に出るはず
    
    // 型推論テスト
    const preset = engine.preset('thumbnail');
    // preset.format は OutputFormat型として推論されるべき
    // preset.quality は number | undefined として推論されるべき
}

// エラーハンドリングテスト
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
