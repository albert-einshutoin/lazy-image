# v1.3.0 release evidence

Revision: 240d23e061af209d0c34b12feaf93721319a2e05; dirty: false.

Compiler: 13/13 supported inputs accepted; 2 expected rejections.
Strict compiler budgets: 11/11.
Compiler E2E ms: p50 366.016, p90 494.014, worst 599.731.

Three trials in fresh processes; sampled RSS includes process overhead. Timing is local evidence, not a universal speed ranking.
Quality uses common source pixels composited on white and a finite grid. Unmet is retained; SSIM does not establish human perceptual equivalence.

| Case | Format | SSIM-floor bytes lazy / sharp | Byte-cap SSIM lazy / sharp |
|---|---|---:|---:|
| jpeg-verification-io | jpeg | 15780 / 12945 | 0.90847 / 0.90992 |
| jpeg-verification-io | webp | 12536 / 12638 | 0.94209 / 0.93425 |
| jpeg-verification-io | avif | 11962 / 10326 | 0.95290 / 0.96077 |
| orientation-absent-exif | jpeg | 15780 / 12945 | 0.90847 / 0.90992 |
| orientation-absent-exif | webp | 12536 / 12638 | 0.94209 / 0.93425 |
| orientation-absent-exif | avif | 11962 / 10326 | 0.95290 / 0.96077 |
| transparent-avif | jpeg | 336 / 325 | unmet / 1.00000 |
| transparent-avif | webp | 70 / 68 | 1.00000 / 1.00000 |
| transparent-avif | avif | 327 / 285 | unmet / 1.00000 |
| coffee | jpeg | 16078 / 13698 | 0.93965 / 0.94087 |
| coffee | webp | 13760 / 13350 | 0.95592 / 0.97040 |
| coffee | avif | 12754 / 10481 | 0.95204 / 0.96365 |
| chelsea | jpeg | 12566 / 10722 | 0.96280 / 0.96346 |
| chelsea | webp | 12534 / 10096 | 0.97168 / 0.97240 |
| chelsea | avif | 9469 / 7912 | 0.95923 / 0.96968 |
| ui-1 | jpeg | 5823 / 5303 | 0.98805 / 0.98971 |
| ui-1 | webp | 3080 / 2812 | 0.99812 / 0.99830 |
| ui-1 | avif | 6042 / 2024 | 0.90933 / 0.99947 |
| illustration-1 | jpeg | 5136 / 4644 | 0.99206 / 0.99214 |
| illustration-1 | webp | 2794 / 2406 | 0.99442 / 0.99468 |
| illustration-1 | avif | 4268 / 1993 | 0.99469 / 0.99777 |
| alpha-1 | jpeg | 1148 / 1139 | 0.99127 / 0.99397 |
| alpha-1 | webp | 482 / 460 | 0.98886 / 0.98902 |
| alpha-1 | avif | unmet / 537 | 0.99422 / 0.99595 |
| metadata-1 | jpeg | 6328 / 5893 | 0.98810 / 0.98906 |
| metadata-1 | webp | 2556 / 2614 | 0.99744 / 0.99742 |
| metadata-1 | avif | 5204 / 2184 | 0.93468 / 0.99859 |
| ui-2 | jpeg | 5364 / 4975 | 0.98727 / 0.98859 |
| ui-2 | webp | 2828 / 2670 | 0.99808 / 0.99812 |
| ui-2 | avif | 5833 / 1968 | 0.88436 / 0.99951 |
| illustration-2 | jpeg | 3882 / 3489 | 0.99244 / 0.99379 |
| illustration-2 | webp | 1728 / 1828 | 0.99771 / 0.99757 |
| illustration-2 | avif | 3143 / 1624 | 0.99716 / 0.99975 |
| alpha-2 | jpeg | 1447 / 1322 | 0.98915 / 0.98956 |
| alpha-2 | webp | 604 / 612 | 0.99488 / 0.99470 |
| alpha-2 | avif | unmet / 612 | 0.96875 / 0.99894 |
| metadata-2 | jpeg | 5353 / 4930 | 0.98692 / 0.98839 |
| metadata-2 | webp | 2900 / 2648 | 0.99550 / 0.99658 |
| metadata-2 | avif | 6751 / 2054 | 0.88330 / 0.99853 |
