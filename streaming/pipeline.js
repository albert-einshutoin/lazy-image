const fs = require('fs');
const path = require('path');
const os = require('os');
const { PassThrough } = require('stream');

/**
 * Create a disk-backed, bounded-memory pipeline that accepts a Writable input stream
 * and exposes a Readable output stream.
 *
 * Notes:
 * - This is NOT true chunk-by-chunk encoding; input is staged to a temp file first.
 * - Memory stays ~O(1) while disk usage mirrors input/output sizes.
 * - Name is kept for backward compatibility; future true streaming APIs will be additive.
 *
 * options:
 * - format: 'jpeg' | 'png' | 'webp' | 'avif'
 * - quality?: number
 * - ops: array of operations (same schema as core API)
 */
function createStreamingPipeline(options) {
    const { format = 'jpeg', quality, ops = [], ImageEngine } = options ?? {};
    const Engine = ImageEngine || require('../index').ImageEngine;
    if (!Engine) {
        throw new Error('ImageEngine must be provided to createStreamingPipeline');
    }
    const tempBase = fs.mkdtempSync(path.join(os.tmpdir(), 'lazy-image-stream-'));
    const inputPath = path.join(tempBase, 'input.bin');
    const outputPath = path.join(tempBase, 'output.bin');
    let cleaned = false;

    const writable = fs.createWriteStream(inputPath);
    const readable = new PassThrough();

    async function process() {
        try {
            let engine = Engine.fromPath(inputPath);
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
            // release reference ASAP to allow underlying mmap to close on platforms that keep file handles open
            engine = null;

            const rs = fs.createReadStream(outputPath);
            rs.on('error', (err) => {
                readable.destroy(err);
                cleanup();
            });
            rs.on('close', cleanup);
            readable.on('error', cleanup);
            readable.on('close', cleanup);
            rs.pipe(readable);
        } catch (err) {
            readable.destroy(err);
            cleanup();
        }
    }

    function cleanup() {
        if (cleaned) return;
        cleaned = true;
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
