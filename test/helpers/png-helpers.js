/**
 * Shared PNG construction helpers for tests.
 *
 * These utilities build minimal, valid PNG buffers from scratch so that
 * integration tests can exercise codec and firewall paths without
 * depending on fixture files on disk.
 */

const zlib = require('zlib');

function buildCrc32Table() {
    const table = new Uint32Array(256);
    for (let i = 0; i < 256; i++) {
        let c = i;
        for (let k = 0; k < 8; k++) {
            c = (c & 1) ? (0xEDB88320 ^ (c >>> 1)) : (c >>> 1);
        }
        table[i] = c >>> 0;
    }
    return table;
}

const CRC_TABLE = buildCrc32Table();

function crc32(buf) {
    let crc = 0xFFFFFFFF;
    for (const byte of buf) {
        crc = (crc >>> 8) ^ CRC_TABLE[(crc ^ byte) & 0xFF];
    }
    return (crc ^ 0xFFFFFFFF) >>> 0;
}

function pngChunk(type, data) {
    const typeBuf = Buffer.from(type, 'ascii');
    const lengthBuf = Buffer.alloc(4);
    lengthBuf.writeUInt32BE(data.length, 0);
    const crcBuf = Buffer.alloc(4);
    const crc = crc32(Buffer.concat([typeBuf, data]));
    crcBuf.writeUInt32BE(crc, 0);
    return Buffer.concat([lengthBuf, typeBuf, data, crcBuf]);
}

function createGrayscalePng(width, height) {
    const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(width, 0);
    ihdr.writeUInt32BE(height, 4);
    ihdr[8] = 8; // bit depth
    ihdr[9] = 0; // color type: grayscale
    ihdr[10] = 0; // compression
    ihdr[11] = 0; // filter
    ihdr[12] = 0; // interlace

    const rowSize = width + 1;
    const raw = Buffer.alloc(rowSize * height);
    for (let y = 0; y < height; y++) {
        raw[y * rowSize] = 0; // filter type 0
    }

    const compressed = zlib.deflateSync(raw);
    const ihdrChunk = pngChunk('IHDR', ihdr);
    const idatChunk = pngChunk('IDAT', compressed);
    const iendChunk = pngChunk('IEND', Buffer.alloc(0));
    return Buffer.concat([signature, ihdrChunk, idatChunk, iendChunk]);
}

function createRgbaPng(width, height, rgba = [255, 0, 0, 128]) {
    const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
    const ihdr = Buffer.alloc(13);
    ihdr.writeUInt32BE(width, 0);
    ihdr.writeUInt32BE(height, 4);
    ihdr[8] = 8;
    ihdr[9] = 6; // RGBA
    const pixel = Buffer.from(rgba);
    if (pixel.length !== 4) {
        throw new RangeError('rgba must contain exactly four channel values');
    }
    const raw = Buffer.alloc((width * 4 + 1) * height);
    for (let y = 0; y < height; y++) {
        const row = y * (width * 4 + 1);
        raw[row] = 0;
        for (let x = 0; x < width; x++) {
            pixel.copy(raw, row + 1 + x * 4);
        }
    }
    return Buffer.concat([
        signature,
        pngChunk('IHDR', ihdr),
        pngChunk('IDAT', zlib.deflateSync(raw)),
        pngChunk('IEND', Buffer.alloc(0)),
    ]);
}

function createApng(width = 2, height = 2) {
    const png = createRgbaPng(width, height);
    const afterIhdr = 8 + 12 + 13;
    const animationControl = Buffer.alloc(8);
    animationControl.writeUInt32BE(1, 0); // one animation frame
    animationControl.writeUInt32BE(0, 4); // loop forever
    return Buffer.concat([
        png.subarray(0, afterIhdr),
        pngChunk('acTL', animationControl),
        png.subarray(afterIhdr),
    ]);
}

module.exports = {
    buildCrc32Table,
    crc32,
    pngChunk,
    createGrayscalePng,
    createRgbaPng,
    createApng,
};
