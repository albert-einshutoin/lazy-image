const crypto = require('crypto');
const sharp = require('sharp');

/**
 * RGBAデコードとメタデータ取得
 * ヘッダー差異を排除し、純粋なピクセルバイトのみを扱う
 */
async function decodeToRgba(buffer) {
    return sharp(buffer)
        .ensureAlpha()
        .raw()
        .toBuffer({ resolveWithObject: true });
}

/**
 * ピクセルハッシュ（SHA-256）を計算し、幅・高さ情報も返す
 */
async function hashPixels(buffer) {
    const { data, info } = await decodeToRgba(buffer);
    const hash = crypto.createHash('sha256').update(data).digest('hex');
    return { hash, info };
}

/**
 * ImageEngine に対してケース定義のオペレーションを適用
 */
function applyOperationsToEngine(engine, operations) {
    let pipeline = engine;
    for (const op of operations) {
        switch (op.op) {
            case 'resize':
                pipeline = pipeline.resize(
                    op.width ?? null,
                    op.height ?? null,
                    op.fit ?? 'inside',
                );
                break;
            case 'rotate':
                pipeline = pipeline.rotate(op.degrees);
                break;
            case 'flipH':
                pipeline = pipeline.flipH();
                break;
            case 'flipV':
                pipeline = pipeline.flipV();
                break;
            case 'grayscale':
                pipeline = pipeline.grayscale();
                break;
            case 'autoOrient':
                pipeline = pipeline.autoOrient(op.enabled !== false);
                break;
            default:
                throw new Error(`Unsupported operation for engine: ${op.op}`);
        }
    }
    return pipeline;
}

/**
 * sharp を用いたリファレンス出力を生成
 * ゴールデンの品質比較用に同じオペレーションを適用する
 */
async function renderSharpReference(inputBuffer, operations, output) {
    let pipeline = sharp(inputBuffer);

    for (const op of operations) {
        switch (op.op) {
            case 'resize':
                pipeline = pipeline.resize(op.width ?? null, op.height ?? null, {
                    fit: op.fit ?? 'inside',
                });
                break;
            case 'rotate':
                pipeline = pipeline.rotate(op.degrees);
                break;
            case 'flipH':
                pipeline = pipeline.flop();
                break;
            case 'flipV':
                pipeline = pipeline.flip();
                break;
            case 'grayscale':
                pipeline = pipeline.grayscale();
                break;
            case 'autoOrient':
                pipeline = pipeline.rotate(); // EXIFを尊重するsharpの慣習
                break;
            default:
                throw new Error(`Unsupported operation for sharp: ${op.op}`);
        }
    }

    switch (output.format) {
        case 'jpeg':
            pipeline = pipeline.jpeg({ quality: output.quality, mozjpeg: true });
            break;
        case 'png':
            pipeline = pipeline.png();
            break;
        case 'webp':
            pipeline = pipeline.webp({ quality: output.quality });
            break;
        case 'avif':
            pipeline = pipeline.avif({ quality: output.quality });
            break;
        default:
            throw new Error(`Unsupported format: ${output.format}`);
    }

    return pipeline.toBuffer();
}

module.exports = {
    decodeToRgba,
    hashPixels,
    applyOperationsToEngine,
    renderSharpReference,
};
