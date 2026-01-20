const { resolveFixture } = require('../helpers/paths');

/**
 * ゴールデンテスト用の代表ケース定義
 * - operations: ImageEngine / sharp 両方に適用可能な簡易DSL
 * - output: { format, quality }
 * - thresholds: SSIM/PSNRの下限
 */
module.exports = [
    {
        name: 'jpeg_resize_inside_1200_q82',
        description: '5000px級JPEGを1200pxに圧縮リサイズ',
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
        description: '高解像度PNGを16:9 cover → 回転 → グレースケール',
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
        description: '5000px級JPEG→WebP変換でサイズ優位を確認',
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
        description: 'PNG coverリサイズ（品質のみ比較・グレースケールなし）',
        input: resolveFixture('test_4.5MB_5000x5000.png'),
        operations: [
            { op: 'resize', width: 1600, height: 900, fit: 'cover' },
        ],
        output: { format: 'png' },
        thresholds: { minSsim: 0.98, minPsnr: 30 },
        sizeRatioMax: 0.6,
    },
];
