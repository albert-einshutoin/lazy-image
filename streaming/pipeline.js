const fs = require('fs');
const path = require('path');
const os = require('os');
const { PassThrough } = require('stream');
const { ImageEngine } = require('../index');

/**
 * Create a streaming processor that accepts a Writable input stream and exposes a Readable output stream.
 * Internally it stages input to a temp file (bounded memory), then processes via ImageEngine.fromPath,
 * writes to another temp file, and streams the result out.
 *
 * options:
 * - format: 'jpeg' | 'png' | 'webp' | 'avif'
 * - quality?: number
 * - ops: array of operations (same schema as core API)
 */
function createStreamingPipeline(options) {
    const { format = 'jpeg', quality, ops = [] } = options ?? {};
    const tempBase = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-stream-'));
    const inputPath = path.join(tempBase, 'input.bin');
    const outputPath = path.join(tempBase, 'output.bin');

    const writable = fs.createWriteStream(inputPath);
    const readable = new PassThrough();

    async function process() {
        try {
            let engine = ImageEngine.fromPath(inputPath);
            for (const op of ops) {
                switch (op.op) {
                    case 'resize':
                        engine = engine.resize(op.width ?? null, op.height ?? null, op.fit ?? null);
                        break;
                    case 'rotate':
                        engine = engine.rotate(op.degrees);
                        break;
                    case 'flipH':
                        engine = engine.flipH();
                        break;
                    case 'flipV':
                        engine = engine.flipV();
                        break;
                    case 'grayscale':
                        engine = engine.grayscale();
                        break;
                    case 'autoOrient':
                        engine = engine.autoOrient(op.enabled !== false);
                        break;
                    default:
                        throw new Error(`Unsupported op: ${op.op}`);
                }
            }
            await engine.toFile(outputPath, format, quality ?? undefined);
            const rs = fs.createReadStream(outputPath);
            rs.on('error', (err) => {
                readable.destroy(err);
                cleanup();
            });
            readable.on('error', cleanup);
            rs.pipe(readable).on('finish', cleanup);
        } catch (err) {
            readable.destroy(err);
            cleanup();
        }
    }

    function cleanup() {
        fs.rm(inputPath, { force: true }, () => {});
        fs.rm(outputPath, { force: true }, () => {});
        fs.rm(tempBase, { force: true, recursive: true }, () => {});
    }

    writable.on('finish', () => {
        process();
    });
    writable.on('error', (err) => {
        readable.destroy(err);
        cleanup();
    });

    return { writable, readable };
}

module.exports = {
    createStreamingPipeline,
};
