const { resolveFixture } = require('../helpers/paths');

/**
 * ゴールデンテスト用の代表ケース定義
 * - operations: ImageEngine / sharp 両方に適用可能な簡易DSL
 * - output: { format, quality }
 * - thresholds: SSIM/PSNRの下限
 */
module.exports = [
    {
        name: 'jpeg_resize_inside_800_q82',
        description: '基礎のJPEGリサイズ（insideフィット）',
        input: resolveFixture('test_input.jpg'),
        operations: [
            { op: 'resize', width: 800, height: null, fit: 'inside' },
        ],
        output: { format: 'jpeg', quality: 82 },
        thresholds: { minSsim: 0.998, minPsnr: 45 },
    },
    {
        name: 'png_cover_rotate_grayscale',
        description: 'PNGでcoverリサイズ→回転→グレースケール（ロスレス期待）',
        input: resolveFixture('test_input.jpg'),
        operations: [
            { op: 'resize', width: 480, height: 360, fit: 'cover' },
            { op: 'rotate', degrees: 180 },
            { op: 'grayscale' },
        ],
        output: { format: 'png' },
        thresholds: { minSsim: 0.9995, minPsnr: 48 },
    },
    {
        name: 'webp_cover_flip',
        description: 'WebPでcoverリサイズ＋水平反転',
        input: resolveFixture('test_input.jpg'),
        operations: [
            { op: 'resize', width: 640, height: 426, fit: 'cover' },
            { op: 'flipH' },
        ],
        output: { format: 'webp', quality: 80 },
        thresholds: { minSsim: 0.997, minPsnr: 42 },
    },
    {
        name: 'avif_inside_large_q60',
        description: 'AVIF高解像度リサイズ（insideフィット）',
        input: resolveFixture('test_input.jpg'),
        operations: [
            { op: 'resize', width: 1280, height: null, fit: 'inside' },
        ],
        output: { format: 'avif', quality: 60 },
        thresholds: { minSsim: 0.995, minPsnr: 40 },
    },
];
