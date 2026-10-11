# Case tables (generated)

Source: 37777bbc0b4988e888390e72a9093f602b8908ea; checkout native 1.4.2. See EVALUATION.md for interpretation.

## Compiler: one combined policy job per case

| Case / category | Formats / widths | Outcome | E2E p50 / p90 ms | Sampled RSS MiB |
|---|---|---|---:|---:|
| jpeg-verification-io / photo | jpeg / 320,1057 | accepted | 246.069 / 250.341 | 121.41 |
| orientation-absent-exif / metadata | webp / 320 | accepted | 128.569 / 135.890 | 115.71 |
| transparent-avif / alpha | webp,avif / 32 | accepted | 19.667 / 19.795 | 91.23 |
| coffee / photo | jpeg,webp,avif / 160 | accepted | 966.965 / 985.077 | 113.77 |
| chelsea / photo | jpeg,webp,avif / 160 | accepted | 711.723 / 712.179 | 112.89 |
| ui-1 / ui | jpeg,webp,avif / 160 | accepted | 551.575 / 563.463 | 112.19 |
| illustration-1 / illustration | jpeg,webp,avif / 160 | accepted | 660.594 / 663.791 | 110.41 |
| alpha-1 / alpha | webp,avif / 160 | accepted | 500.074 / 500.904 | 93.11 |
| metadata-1 / metadata | jpeg,webp,avif / 160 | accepted | 676.130 / 677.568 | 110.05 |
| hostile-1 / hostile | webp / 160 | expected-rejection | 6.339 / 6.491 | 79.45 |
| ui-2 / ui | jpeg,webp,avif / 160 | accepted | 515.253 / 516.043 | 110.86 |
| illustration-2 / illustration | jpeg,webp,avif / 160 | accepted | 575.904 / 576.031 | 110.65 |
| alpha-2 / alpha | webp,avif / 160 | accepted | 640.008 / 646.385 | 90.74 |
| metadata-2 / metadata | jpeg,webp,avif / 160 | accepted | 530.306 / 531.549 | 113.13 |
| hostile-2 / hostile | webp / 160 | expected-rejection | 6.738 / 7.074 | 79.57 |

## Codec: independent quality-floor and byte-cap selections

Bytes/time columns use each engine's smallest candidate meeting the shared floor. Cap column gives highest SSIM inside the shared cap; PSNR is dB. Unmet is not a processing failure. RSS is the whole grid worker, not per-candidate allocation.

| Case / category | Format | Floor / cap B | Floor bytes lazy / sharp | Cap SSIM / PSNR lazy ; sharp | Floor encode p50 ms lazy / sharp | Grid RSS MiB lazy / sharp |
|---|---|---:|---:|---|---:|---:|
| jpeg-verification-io / photo | jpeg | 0.909925 / 12946 | unmet / 12946 @q80 | 0.895732 / 33.975 ; 0.909925 / 34.488 | floor unmet / 12.240 | 176.12 / 150.00 |
| jpeg-verification-io / photo | webp | 0.934252 / 12638 | 12536 @q80 / 12638 @q80 | 0.942086 / 35.275 ; 0.934252 / 35.085 | 56.991 / 14.388 | 178.51 / 162.75 |
| jpeg-verification-io / photo | avif | 0.961334 / 10129 | 11967 @q60 / 10129 @q60 | 0.952869 / 35.154 ; 0.961334 / 38.280 | 121.921 / 511.639 | 198.46 / 165.64 |
| orientation-absent-exif / metadata | jpeg | 0.909925 / 12946 | unmet / 12946 @q80 | 0.895732 / 33.975 ; 0.909925 / 34.488 | floor unmet / 12.041 | 172.87 / 150.70 |
| orientation-absent-exif / metadata | webp | 0.934252 / 12638 | 12536 @q80 / 12638 @q80 | 0.942086 / 35.275 ; 0.934252 / 35.085 | 56.464 / 14.570 | 179.20 / 165.32 |
| orientation-absent-exif / metadata | avif | 0.961334 / 10129 | 11967 @q60 / 10129 @q60 | 0.952869 / 35.154 ; 0.961334 / 38.280 | 121.925 / 516.231 | 206.62 / 158.93 |
| transparent-avif / alpha | jpeg | 1.000000 / 326 | 337 @q10 / 325 @q30 | unmet ; 1.000000 / 46.370 | 1.556 / 1.312 | 106.17 / 87.91 |
| transparent-avif / alpha | webp | 1.000000 / 76 | 70 @q20 / 68 @q20 | 1.000000 / 40.512 ; 1.000000 / 47.628 | 1.262 / 1.068 | 105.22 / 87.06 |
| transparent-avif / alpha | avif | 1.000000 / 288 | 327 @q100 / 285 @q10 | unmet ; 1.000000 / 54.151 | 1.118 / 4.530 | 122.46 / 99.61 |
| coffee / photo | jpeg | 0.940866 / 13698 | unmet / 13698 @q80 | 0.928983 / 32.241 ; 0.940866 / 32.898 | floor unmet / 11.267 | 153.06 / 133.77 |
| coffee / photo | webp | 0.970396 / 13350 | 13760 @q80 / 13350 @q80 | 0.955916 / 33.545 ; 0.970396 / 34.905 | 39.237 / 11.146 | 153.27 / 138.93 |
| coffee / photo | avif | 0.964590 / 10417 | 12754 @q60 / 10417 @q60 | 0.952037 / 33.765 ; 0.964590 / 36.003 | 144.825 / 513.039 | 173.93 / 136.78 |
| chelsea / photo | jpeg | 0.963458 / 10722 | unmet / 10722 @q80 | 0.955478 / 36.188 ; 0.963458 / 36.962 | floor unmet / 9.967 | 154.73 / 133.38 |
| chelsea / photo | webp | 0.972364 / 10082 | 12558 @q85 / 10082 @q80 | 0.971534 / 37.435 ; 0.972364 / 37.589 | 32.413 / 10.889 | 152.04 / 139.91 |
| chelsea / photo | avif | 0.969889 / 7809 | 9519 @q60 / 7809 @q60 | 0.958558 / 36.237 ; 0.969889 / 37.703 | 76.007 / 382.469 | 176.87 / 151.98 |
| ui-1 / ui | jpeg | 0.989706 / 5303 | unmet / 5303 @q80 | 0.987631 / 36.306 ; 0.989706 / 37.361 | floor unmet / 6.865 | 157.21 / 145.71 |
| ui-1 / ui | webp | 0.998298 / 2812 | 3080 @q85 / 2812 @q80 | 0.998121 / 41.271 ; 0.998298 / 41.510 | 18.763 / 6.526 | 167.18 / 145.47 |
| ui-1 / ui | avif | 0.999495 / 1920 | 6042 @q80 / 1920 @q60 | 0.909334 / 30.096 ; 0.999495 / 44.553 | 92.573 / 161.177 | 182.02 / 160.16 |
| illustration-1 / illustration | jpeg | 0.992139 / 4644 | unmet / 4644 @q80 | 0.990485 / 35.641 ; 0.992139 / 36.030 | floor unmet / 6.245 | 164.93 / 137.15 |
| illustration-1 / illustration | webp | 0.994683 / 2406 | 2794 @q85 / 2406 @q80 | 0.994422 / 36.774 ; 0.994683 / 36.907 | 23.217 / 6.910 | 174.71 / 150.55 |
| illustration-1 / illustration | avif | 0.997700 / 1801 | 4268 @q95 / 1801 @q60 | 0.994692 / 37.370 ; 0.997700 / 48.376 | 199.835 / 162.216 | 187.52 / 154.28 |
| alpha-1 / alpha | jpeg | 0.993967 / 1139 | unmet / 1139 @q80 | 0.992599 / 48.025 ; 0.993967 / 49.176 | floor unmet / 2.808 | 117.39 / 97.34 |
| alpha-1 / alpha | webp | 0.989015 / 460 | 482 @q80 / 460 @q80 | 0.988863 / 45.176 ; 0.989015 / 46.101 | 10.254 / 4.420 | 119.58 / 98.85 |
| alpha-1 / alpha | avif | 0.995883 / 535 | unmet / 535 @q60 | 0.994217 / 49.384 ; 0.995883 / 50.502 | floor unmet / 32.470 | 138.45 / 106.57 |
| metadata-1 / metadata | jpeg | 0.989059 / 5893 | unmet / 5893 @q80 | 0.987017 / 36.607 ; 0.989059 / 37.991 | floor unmet / 6.905 | 156.21 / 137.03 |
| metadata-1 / metadata | webp | 0.997416 / 2614 | 2556 @q80 / 2614 @q80 | 0.997442 / 43.460 ; 0.997416 / 43.595 | 24.393 / 6.715 | 168.57 / 150.87 |
| metadata-1 / metadata | avif | 0.998547 / 2078 | 5204 @q70 / 2078 @q60 | 0.909396 / 30.439 ; 0.998547 / 45.670 | 83.944 / 162.510 | 193.19 / 169.11 |
| ui-2 / ui | jpeg | 0.988590 / 4975 | unmet / 4975 @q80 | 0.984094 / 35.806 ; 0.988590 / 36.700 | floor unmet / 7.473 | 154.80 / 136.96 |
| ui-2 / ui | webp | 0.998123 / 2670 | 2828 @q85 / 2670 @q80 | 0.998080 / 42.456 ; 0.998123 / 42.665 | 13.702 / 6.425 | 176.78 / 152.22 |
| ui-2 / ui | avif | 0.999510 / 1869 | 5833 @q85 / 1869 @q60 | 0.884360 / 29.823 ; 0.999510 / 46.579 | 143.806 / 142.086 | 182.43 / 166.62 |
| illustration-2 / illustration | jpeg | 0.993790 / 3489 | unmet / 3489 @q80 | 0.991597 / 37.300 ; 0.993790 / 38.030 | floor unmet / 5.707 | 164.47 / 145.90 |
| illustration-2 / illustration | webp | 0.997570 / 1828 | 1728 @q80 / 1828 @q80 | 0.997714 / 39.503 ; 0.997570 / 39.648 | 16.406 / 5.952 | 176.17 / 148.02 |
| illustration-2 / illustration | avif | 0.999642 / 1482 | 2624 @q85 / 1343 @q50 | 0.994418 / 39.731 ; 0.999709 / 49.058 | 138.568 / 131.827 | 180.40 / 171.12 |
| alpha-2 / alpha | jpeg | 0.989560 / 1322 | unmet / 1322 @q80 | 0.987269 / 40.124 ; 0.989560 / 41.033 | floor unmet / 2.748 | 116.80 / 98.21 |
| alpha-2 / alpha | webp | 0.994697 / 612 | 604 @q80 / 612 @q80 | 0.994879 / 43.161 ; 0.994697 / 43.329 | 6.057 / 2.491 | 118.89 / 97.73 |
| alpha-2 / alpha | avif | 0.998938 / 604 | unmet / 604 @q60 | 0.968749 / 36.361 ; 0.998938 / 53.831 | floor unmet / 46.221 | 136.34 / 108.40 |
| metadata-2 / metadata | jpeg | 0.988392 / 4930 | unmet / 4930 @q80 | 0.984019 / 36.253 ; 0.988392 / 37.493 | floor unmet / 6.690 | 161.10 / 137.80 |
| metadata-2 / metadata | webp | 0.996580 / 2648 | 2900 @q85 / 2648 @q80 | 0.995496 / 44.353 ; 0.996580 / 44.852 | 17.563 / 6.071 | 168.41 / 144.54 |
| metadata-2 / metadata | avif | 0.998596 / 1904 | 6751 @q90 / 1904 @q60 | 0.883297 / 29.828 ; 0.998596 / 47.159 | 156.503 / 146.818 | 190.07 / 162.06 |

Display rounding does not alter selection targets; raw values/trial samples remain in case-summary.json and raw JSON.
