const sharp = require('sharp');
const { ssim } = require('ssim.js');

/**
 * Decode an encoded image buffer into RGBA raw pixels.
 * @param {Buffer} buffer
 * @returns {Promise<{data: Uint8Array, info: {width:number,height:number,channels:number}}>}
 */
async function decodeToRgba(buffer) {
    return sharp(buffer)
        .ensureAlpha()
        .raw()
        .toBuffer({ resolveWithObject: true });
}

/**
 * Calculate PSNR (Peak Signal-to-Noise Ratio) between two image buffers.
 * Assumes same dimensions; throws if mismatch.
 * @param {Buffer} a
 * @param {Buffer} b
 * @returns {Promise<number>} PSNR in dB (Infinity if identical)
 */
async function calculatePsnr(a, b) {
    const [imgA, imgB] = await Promise.all([decodeToRgba(a), decodeToRgba(b)]);
    if (imgA.info.width !== imgB.info.width || imgA.info.height !== imgB.info.height) {
        throw new Error('PSNR: dimensions differ');
    }

    const dataA = imgA.data;
    const dataB = imgB.data;
    let mse = 0;
    for (let i = 0; i < dataA.length; i++) {
        const diff = dataA[i] - dataB[i];
        mse += diff * diff;
    }
    mse /= dataA.length;
    if (mse === 0) return Infinity;
    const maxI = 255;
    return 10 * Math.log10((maxI * maxI) / mse);
}

/**
 * Calculate SSIM between two image buffers using ssim.js
 * @param {Buffer} a
 * @param {Buffer} b
 * @returns {Promise<number>} SSIM in [0,1]
 */
async function calculateSsim(a, b) {
    const [imgA, imgB] = await Promise.all([decodeToRgba(a), decodeToRgba(b)]);
    if (imgA.info.width !== imgB.info.width || imgA.info.height !== imgB.info.height) {
        throw new Error('SSIM: dimensions differ');
    }
    const dataA = new Uint8ClampedArray(imgA.data.buffer, imgA.data.byteOffset, imgA.data.byteLength);
    const dataB = new Uint8ClampedArray(imgB.data.buffer, imgB.data.byteOffset, imgB.data.byteLength);
    const result = await ssim(
        { data: dataA, width: imgA.info.width, height: imgA.info.height },
        { data: dataB, width: imgB.info.width, height: imgB.info.height },
        { windowSize: 8 }
    );
    return result.ssim ?? result.mssim ?? result.mean ?? result.value;
}

/**
 * Compute both SSIM and PSNR.
 */
async function calculateQualityMetrics(a, b) {
    const [psnr, ssimValue] = await Promise.all([calculatePsnr(a, b), calculateSsim(a, b)]);
    return { psnr, ssim: ssimValue };
}

module.exports = {
    calculatePsnr,
    calculateSsim,
    calculateQualityMetrics,
};
