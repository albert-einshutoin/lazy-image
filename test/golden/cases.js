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
        description: '中解像度WebPをスクエアcover＋水平反転',
        input: resolveFixture('test_90KB_1471x1471.webp'),
        operations: [
            { op: 'resize', width: 800, height: 800, fit: 'cover' },
            { op: 'flipH' },
        ],
        output: { format: 'webp', quality: 80 },
        thresholds: { minSsim: 0.99, minPsnr: 34 },
        sizeRatioMax: 1.1,
    },
];
