const { resolveFixture } = require('../helpers/paths');

/**
 * Golden test case definitions
 * - operations: Simple DSL applicable to both ImageEngine and sharp
 * - output: { format, quality }
 * - thresholds: Minimum SSIM/PSNR values
 */
module.exports = [
    {
        name: 'jpeg_resize_inside_1200_q82',
        description: 'Resize 5000px JPEG to 1200px with compression',
        input: resolveFixture('test_3.2MB_5000x5000.jpg'),
        operations: [
            { op: 'resize', width: 1200, height: null, fit: 'inside' },
        ],
        output: { format: 'jpeg', quality: 82 },
        thresholds: { minSsim: 0.99, minPsnr: 38 },
        sizeRatioMax: 0.95, // lazy-image output should be <= 95% of sharp reference
    },
    {
        name: 'png_cover_rotate_grayscale',
        description: 'High-resolution PNG: 16:9 cover → rotate → grayscale',
        input: resolveFixture('test_4.5MB_5000x5000.png'),
        operations: [
            { op: 'resize', width: 1600, height: 900, fit: 'cover' },
            { op: 'rotate', degrees: 180 },
            { op: 'grayscale' },
        ],
        output: { format: 'png' },
        thresholds: { minSsim: 0.90, minPsnr: 20 },
    },
    {
        name: 'webp_cover_flip',
        description: 'Convert 5000px JPEG to WebP to verify size advantage',
        input: resolveFixture('test_3.2MB_5000x5000.jpg'),
        operations: [
            { op: 'resize', width: 800, height: 800, fit: 'cover' },
            { op: 'flipH' },
        ],
        output: { format: 'webp', quality: 78 },
        thresholds: { minSsim: 0.99, minPsnr: 40 },
        sizeRatioMax: 0.97,
    },
    {
        name: 'png_cover_no_grayscale',
        description: 'PNG cover resize (quality comparison only, no grayscale)',
        input: resolveFixture('test_4.5MB_5000x5000.png'),
        operations: [
            { op: 'resize', width: 1600, height: 900, fit: 'cover' },
        ],
        output: { format: 'png' },
        thresholds: { minSsim: 0.98, minPsnr: 30 },
        sizeRatioMax: 0.6,
    },
    {
        name: 'avif_cover_rotate_grayscale',
        description: 'PNG→AVIF cover resize + 180° rotation + grayscale',
        input: resolveFixture('test_4.5MB_5000x5000.png'),
        operations: [
            { op: 'resize', width: 1200, height: 800, fit: 'cover' },
            { op: 'rotate', degrees: 180 },
            { op: 'grayscale' },
        ],
        output: { format: 'avif', quality: 60 },
        thresholds: { minSsim: 0.90, minPsnr: 22 },
        sizeRatioMax: 1.6,
    },
    {
        name: 'jpeg_to_png_flipv',
        description: 'JPEG→PNG conversion + resize + vertical flip (pixel hash verification, reduced memory)',
        input: resolveFixture('test_3.2MB_5000x5000.jpg'),
        operations: [
            { op: 'resize', width: 2000, height: null, fit: 'inside' },
            { op: 'flipV' },
        ],
        output: { format: 'png' },
        thresholds: { minSsim: 0.995, minPsnr: 42 },
    },
];
