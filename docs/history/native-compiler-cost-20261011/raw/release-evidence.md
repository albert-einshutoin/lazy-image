# v1.3.0 release evidence

Revision: 37777bbc0b4988e888390e72a9093f602b8908ea; dirty: false.

Compiler: 13/13 supported inputs accepted; 2 expected rejections.
Strict compiler budgets: 11/11.
Compiler E2E ms: p50 551.575, p90 712.179, worst 985.077.

Three trials in fresh processes; sampled RSS includes process overhead. Timing is local evidence, not a universal speed ranking.
Quality uses common source pixels composited on white and a finite grid. Unmet is retained; SSIM does not establish human perceptual equivalence.

| Case | Format | SSIM-floor bytes lazy / sharp | Byte-cap SSIM lazy / sharp |
|---|---|---:|---:|
| jpeg-verification-io | jpeg | unmet / 12946 | 0.89573 / 0.90992 |
| jpeg-verification-io | webp | 12536 / 12638 | 0.94209 / 0.93425 |
| jpeg-verification-io | avif | 11967 / 10129 | 0.95287 / 0.96133 |
| orientation-absent-exif | jpeg | unmet / 12946 | 0.89573 / 0.90992 |
| orientation-absent-exif | webp | 12536 / 12638 | 0.94209 / 0.93425 |
| orientation-absent-exif | avif | 11967 / 10129 | 0.95287 / 0.96133 |
| transparent-avif | jpeg | 337 / 325 | unmet / 1.00000 |
| transparent-avif | webp | 70 / 68 | 1.00000 / 1.00000 |
| transparent-avif | avif | 327 / 285 | unmet / 1.00000 |
| coffee | jpeg | unmet / 13698 | 0.92898 / 0.94087 |
| coffee | webp | 13760 / 13350 | 0.95592 / 0.97040 |
| coffee | avif | 12754 / 10417 | 0.95204 / 0.96459 |
| chelsea | jpeg | unmet / 10722 | 0.95548 / 0.96346 |
| chelsea | webp | 12558 / 10082 | 0.97153 / 0.97236 |
| chelsea | avif | 9519 / 7809 | 0.95856 / 0.96989 |
| ui-1 | jpeg | unmet / 5303 | 0.98763 / 0.98971 |
| ui-1 | webp | 3080 / 2812 | 0.99812 / 0.99830 |
| ui-1 | avif | 6042 / 1920 | 0.90933 / 0.99949 |
| illustration-1 | jpeg | unmet / 4644 | 0.99049 / 0.99214 |
| illustration-1 | webp | 2794 / 2406 | 0.99442 / 0.99468 |
| illustration-1 | avif | 4268 / 1801 | 0.99469 / 0.99770 |
| alpha-1 | jpeg | unmet / 1139 | 0.99260 / 0.99397 |
| alpha-1 | webp | 482 / 460 | 0.98886 / 0.98902 |
| alpha-1 | avif | unmet / 535 | 0.99422 / 0.99588 |
| metadata-1 | jpeg | unmet / 5893 | 0.98702 / 0.98906 |
| metadata-1 | webp | 2556 / 2614 | 0.99744 / 0.99742 |
| metadata-1 | avif | 5204 / 2078 | 0.90940 / 0.99855 |
| ui-2 | jpeg | unmet / 4975 | 0.98409 / 0.98859 |
| ui-2 | webp | 2828 / 2670 | 0.99808 / 0.99812 |
| ui-2 | avif | 5833 / 1869 | 0.88436 / 0.99951 |
| illustration-2 | jpeg | unmet / 3489 | 0.99160 / 0.99379 |
| illustration-2 | webp | 1728 / 1828 | 0.99771 / 0.99757 |
| illustration-2 | avif | 2624 / 1343 | 0.99442 / 0.99971 |
| alpha-2 | jpeg | unmet / 1322 | 0.98727 / 0.98956 |
| alpha-2 | webp | 604 / 612 | 0.99488 / 0.99470 |
| alpha-2 | avif | unmet / 604 | 0.96875 / 0.99894 |
| metadata-2 | jpeg | unmet / 4930 | 0.98402 / 0.98839 |
| metadata-2 | webp | 2900 / 2648 | 0.99550 / 0.99658 |
| metadata-2 | avif | 6751 / 1904 | 0.88330 / 0.99860 |
