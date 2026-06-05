# Benchmark Results Log

This file stores reproducible benchmark snapshots for new benchmark suites.

## Issue #377 (AVIF/Memory/Cold Start)

Date: 2026-02-16  
Branch: `codex/feature-377-bench-suite`  
Environment: local machine (Node.js + sharp installed)

### Commands

```bash
npm run test:bench:avif
npm run test:bench:memory
npm run test:bench:cold-start
```

### Results

#### AVIF / WebP / JPEG Matrix

| Case | lazy-image avg ms | sharp avg ms | speed ratio (sharp/lazy) | lazy size | sharp size | size diff | Note |
|---|---:|---:|---:|---:|---:|---:|---|
| AVIF q45 | 408.6 | 161.6 | 0.40x | 16.2 KB | 13.6 KB | 18.5% | lower quality / smaller output |
| AVIF q60 | 535.4 | 167.3 | 0.31x | 28.0 KB | 21.7 KB | 29.0% | default quality baseline |
| AVIF q75 | 652.4 | 202.2 | 0.31x | 39.9 KB | 34.1 KB | 17.0% | higher quality / larger output |
| WebP q80 | 178.2 | 79.3 | 0.45x | 31.7 KB | 29.0 KB | 9.3% | WebP baseline |
| JPEG q80 | 69.5 | 71.2 | 1.02x | 30.8 KB | 38.5 KB | -20.0% | JPEG baseline |

#### Memory Tracking (RSS Delta vs Estimate)

| Scenario | Estimated weight (MB) | Avg RSS delta (MB) | Avg metrics.peakRss (MB) | Avg time (ms) | Avg output (KB) | delta/estimate |
|---|---:|---:|---:|---:|---:|---:|
| jpeg-noops-q80 | 113.2 | 7.2 | 326.4 | 674.8 | 1196.2 | 0.06 |
| webp-resize800-q80 | 103.4 | 5.5 | 377.4 | 171.3 | 31.7 | 0.05 |
| jpeg-resize-rotate-gray-q75 | 103.4 | 7.5 | 430.3 | 105.5 | 47.6 | 0.07 |

#### Cold Start (Serverless-Oriented)

| Metric | Avg | p50 | p95 |
|---|---:|---:|---:|
| require() time | 2.9ms | 2.8ms | 4.5ms |
| first toBuffer() time | 30.5ms | 29.9ms | 35.7ms |
| total cold start | 33.5ms | 33.0ms | 40.2ms |

Notes:
- AVIF rows compare quality bands (q45/q60/q75) to expose practical speed/size trade-offs.
- `metrics.peakRss` is process peak RSS (`getrusage`) and not per-operation isolated memory.
- RSS delta is measured as before/after process RSS approximation, mainly for trend monitoring.

## Issue #619 (Benchmark Claim Refresh)

Date: 2026-05-29
Branch: `fix/benchmark-claims-619`
Environment: macOS 26.3, Apple M4 (arm64), Node.js v24.2.0, sharp v0.34.5, lazy-image v0.15.0

### Commands

```bash
npm run test:bench
node --expose-gc test/benchmarks/convert-only.bench.js
npm run test:bench:extended
```

### Canonical No-Resize Conversion

| Conversion | lazy-image | sharp | Speed Ratio (sharp/lazy) | Size Diff |
|------------|-----------:|------:|--------------------------:|----------:|
| PNG -> WebP | 4,323ms / 1,150,954 bytes | 909ms / 1,124,232 bytes | 0.21x | +2.4% |
| PNG -> AVIF | 12,668ms / 1,718,430 bytes | 4,729ms / 1,290,501 bytes | 0.37x | +33.2% |
| PNG -> JPEG | 700ms / 1,224,894 bytes | 660ms / 1,475,223 bytes | 0.94x | -17.0% |

### Resize 800px Comparison

| Scenario | lazy-image | sharp | Speed Ratio (sharp/lazy) | Size Diff |
|----------|-----------:|------:|--------------------------:|----------:|
| PNG -> JPEG resize 800px | 70ms / 31,518 bytes | 79ms / 39,416 bytes | 1.13x | -20.0% |
| PNG -> WebP resize 800px | 166ms / 32,448 bytes | 90ms / 29,686 bytes | 0.54x | +9.3% |
| PNG -> AVIF resize 800px | 442ms / 28,678 bytes | 179ms / 22,227 bytes | 0.40x | +29.0% |
| Resize + rotate + grayscale -> JPEG | 68ms / 27,129 bytes | 112ms / 24,650 bytes | 1.65x | +10.1% |

### Extended AVIF / WebP / JPEG Matrix

| Case | lazy-image avg ms | sharp avg ms | speed ratio (sharp/lazy) | lazy size | sharp size | size diff | Note |
|---|---:|---:|---:|---:|---:|---:|---|
| AVIF q45 | 331.8 | 157.6 | 0.48x | 16.2 KB | 13.6 KB | 18.5% | lower quality / smaller output |
| AVIF q60 | 449.1 | 164.2 | 0.37x | 28.0 KB | 21.7 KB | 29.0% | default quality baseline |
| AVIF q75 | 526.3 | 200.4 | 0.38x | 39.9 KB | 34.1 KB | 17.0% | higher quality / larger output |
| WebP q80 | 161.6 | 77.7 | 0.48x | 31.7 KB | 29.0 KB | 9.3% | WebP baseline |
| JPEG q80 | 57.4 | 71.8 | 1.25x | 30.8 KB | 38.5 KB | -20.0% | JPEG baseline |

### Memory Tracking

| Scenario | Estimated weight (MB) | Avg RSS delta (MB) | Avg metrics.peakRss (MB) | Avg time (ms) | Avg output (KB) | delta/estimate |
|---|---:|---:|---:|---:|---:|---:|
| jpeg-noops-q80 | 113.2 | 6.9 | 326.5 | 653.5 | 1196.2 | 0.06 |
| webp-resize800-q80 | 103.4 | 11.4 | 410.8 | 162.0 | 31.7 | 0.11 |
| jpeg-resize-rotate-gray-q75 | 103.4 | 9.0 | 484.8 | 90.5 | 47.6 | 0.09 |

### Cold Start

| Metric | Avg | p50 | p95 |
|---|---:|---:|---:|
| require() time | 3.0ms | 3.0ms | 3.5ms |
| first toBuffer() time | 28.5ms | 28.4ms | 29.5ms |
| total cold start | 31.5ms | 31.3ms | 32.5ms |

Notes:
- Current public size win is JPEG-specific in these measured scenarios.
- Current AVIF/WebP measurements favor sharp for speed and usually output size.
- Speed ratios use `sharp time / lazy-image time`; values below 1.0 mean sharp was faster.

## Issue #638 (JPEG / jpegli Backend Bake-Off)

Date: 2026-06-04
Branch: `bench/638-jpegli-artifact`
Environment: local darwin/arm64, Node.js v24.2.0, lazy-image v0.15.0

### Tooling

`cjpegli` was built from `google/jpegli` commit
`031a0077f5799a6041004267fc12b956c1f52a20`.
The Homebrew `jpeg-xl` package provided `cjxl`, but did not provide the
JPEG-compatible `cjpegli` binary in this environment.

```bash
git clone --depth 1 https://github.com/google/jpegli.git /tmp/jpegli-src
git -C /tmp/jpegli-src submodule update --init --depth 1
cmake -S /tmp/jpegli-src -B /tmp/jpegli-src/build \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_PREFIX_PATH="$(brew --prefix highway);$(brew --prefix little-cms2);$(brew --prefix libpng);$(brew --prefix jpeg-turbo)" \
  -DBUILD_TESTING=OFF \
  -DJPEGLI_ENABLE_DOXYGEN=OFF \
  -DJPEGLI_ENABLE_MANPAGES=OFF \
  -DJPEGLI_ENABLE_BENCHMARK=OFF \
  -DJPEGLI_ENABLE_JNI=OFF \
  -DJPEGLI_ENABLE_OPENEXR=OFF \
  -DJPEGLI_ENABLE_DEVTOOLS=OFF \
  -DJPEGLI_ENABLE_FUZZERS=OFF \
  -DJPEGLI_ENABLE_TOOLS=ON \
  -DJPEGLI_ENABLE_JPEGLI_LIBJPEG=ON \
  -DJPEGLI_ENABLE_SJPEG=OFF \
  -DJPEGLI_FORCE_SYSTEM_LCMS2=ON \
  -DJPEGLI_FORCE_SYSTEM_HWY=ON
cmake --build /tmp/jpegli-src/build --target cjpegli --config Release -j 4
```

### Command

```bash
JPEGLI_VERSION=031a0077f5799a6041004267fc12b956c1f52a20 \
  npm run test:bench:jpegli -- \
  --cjpegli /tmp/jpegli-src/build/tools/cjpegli \
  --jpegli-version 031a0077f5799a6041004267fc12b956c1f52a20
```

The generated files were written to `artifacts/benchmark/jpegli-bakeoff.*`;
`artifacts/` is intentionally ignored, so this section is the tracked snapshot.

### Results

| Case | Source class | MozJPEG bytes | jpegli bytes | jpegli bytes delta | MozJPEG ms | jpegli ms | jpegli ms delta | SSIM delta | PSNR delta |
|---|---|---:|---:|---:|---:|---:|---:|---:|---:|
| large-png-default-jpeg | large PNG/photo-like | 32664 | 85778 | +162.6% | 28.3 | 16.8 | -40.7% | +0.00267 | +3.00 |
| photo-jpeg-default-jpeg | large JPEG/photo | 32710 | 87951 | +168.9% | 23.5 | 14.4 | -38.9% | +0.00278 | +3.21 |
| small-jpeg-default-jpeg | small JPEG | 14140 | 28418 | +101.0% | 12.0 | 9.5 | -21.2% | +0.00024 | +1.98 |
| mid-png-smaller-output | medium PNG | 15686 | 31518 | +100.9% | 16.0 | 14.5 | -9.2% | +0.02080 | +1.92 |
| generated-gradient-default-jpeg | smooth gradient | 3168 | 6021 | +90.1% | 4.9 | 5.8 | +19.5% | +0.00541 | +2.21 |
| generated-ui-default-jpeg | screenshot/UI | 33700 | 86024 | +155.3% | 30.0 | 18.1 | -39.7% | +0.09103 | +5.21 |
| generated-texture-high-detail | high-detail texture | 77724 | 174348 | +124.3% | 26.4 | 10.5 | -60.3% | +0.00779 | +7.96 |

### Decision

Do not promote jpegli with the current distance mapping. In this first
same-reference bake-off, jpegli improved objective quality and was usually
faster as an external process, but produced 90.1% to 168.9% larger JPEGs than
the current MozJPEG path. The next jpegli investigation should tune distance
against matched-byte or matched-quality targets before considering an internal
backend boundary or production dependency.

## Issue #639 (AVIF / libavif Backend Bake-Off)

Date: 2026-06-05
Branch: `bench/639-avif-backend-evidence`
Environment: local darwin/arm64, Node.js v24.2.0, lazy-image v0.15.0

### Tooling

`avifenc` was built from libavif tag `v1.4.2`, source commit
`c5240fc79fe5c2407e10afd35f5505ef6333ea49`, with system codec libraries:

- AOM/libaom 3.14.1
- rav1e 0.8.1
- SVT-AV1 4.1.0

The current lazy-image production comparison path used Cargo dependencies
`libavif-sys 0.17.0+libavif.1.0.4` and `rav1e 0.7.1`.

```bash
brew install aom rav1e svt-av1 libpng jpeg-turbo
git clone --branch v1.4.2 --depth 1 https://github.com/AOMediaCodec/libavif.git /tmp/libavif-src
cmake -S /tmp/libavif-src -B /tmp/libavif-src/build \
  -DCMAKE_BUILD_TYPE=Release \
  -DBUILD_SHARED_LIBS=ON \
  -DAVIF_BUILD_APPS=ON \
  -DAVIF_BUILD_TESTS=OFF \
  -DAVIF_CODEC_AOM=SYSTEM \
  -DAVIF_CODEC_AOM_ENCODE=ON \
  -DAVIF_CODEC_AOM_DECODE=ON \
  -DAVIF_CODEC_RAV1E=SYSTEM \
  -DAVIF_CODEC_SVT=SYSTEM \
  -DAVIF_ZLIBPNG=SYSTEM \
  -DAVIF_JPEG=SYSTEM \
  -DCMAKE_PREFIX_PATH="$(brew --prefix aom);$(brew --prefix rav1e);$(brew --prefix svt-av1);$(brew --prefix libpng);$(brew --prefix jpeg-turbo)"
cmake --build /tmp/libavif-src/build --target avifenc --config Release -j 4
/tmp/libavif-src/build/avifenc --version
```

`avifenc --version` reported:

```text
Version: 1.4.2 (aom [enc/dec]:3.14.1, rav1e [enc]:0.8.1 (), svt [enc]:v4.1.0)
libyuv : unavailable
```

### Command

```bash
npm run test:bench:avif-backends -- \
  --avifenc /tmp/libavif-src/build/avifenc \
  --jobs 4
```

The generated files were written to
`artifacts/benchmark/avif-backend-bakeoff.*`; `artifacts/` is intentionally
ignored, so this section is the tracked snapshot.

### Settings

- Candidate backends: rav1e, AOM/libaom, SVT-AV1 through libavif/avifenc.
- Quality mapping: `--qcolor` and `--qalpha` use the same lazy-image quality value on libavif's 0..100 quality scale.
- Speed mapping: lazy-image AVIF quality 60 -> speed 8, quality 75 -> speed 7.
- Thread settings: `avifenc --jobs 4`; lazy-image AVIF thread cap reported 8.
- Pixel settings: `--yuv 420 --range full`.
- Tune settings: libavif/codec defaults; no codec-specific `--advanced` tuning.

### Results

| Case | Backend | Bytes | Bytes vs lazy-image | Encode ms | SSIM vs lazy-image | PSNR vs lazy-image | Alpha OK | ICC OK |
|---|---|---:|---:|---:|---:|---:|---|---|
| large-png-q60 | lazy-image current rav1e | 27789 | baseline | 421.1 | baseline | baseline | n/a | n/a |
| large-png-q60 | avifenc rav1e | 26811 | -3.5% | 216.8 | +0.00035 | +0.30 | n/a | n/a |
| large-png-q60 | avifenc AOM | 26031 | -6.3% | 18.7 | -0.00039 | -0.40 | n/a | n/a |
| large-png-q60 | avifenc SVT | 18989 | -31.7% | 19.0 | -0.00092 | -0.93 | n/a | n/a |
| photo-jpeg-q75 | lazy-image current rav1e | 41756 | baseline | 464.1 | baseline | baseline | n/a | n/a |
| photo-jpeg-q75 | avifenc rav1e | 41560 | -0.5% | 259.8 | +0.00037 | +0.36 | n/a | n/a |
| photo-jpeg-q75 | avifenc AOM | 45142 | +8.1% | 28.6 | -0.00001 | -0.01 | n/a | n/a |
| photo-jpeg-q75 | avifenc SVT | 27112 | -35.1% | 22.3 | -0.00075 | -0.43 | n/a | n/a |
| alpha-gradient-q60 | lazy-image current rav1e | 1163 | baseline | 21.5 | baseline | baseline | yes | n/a |
| alpha-gradient-q60 | avifenc rav1e | 1123 | -3.4% | 22.2 | +0.00218 | -0.05 | yes | n/a |
| alpha-gradient-q60 | avifenc AOM | 1483 | +27.5% | 6.3 | -0.00240 | -2.20 | yes | n/a |
| alpha-gradient-q60 | avifenc SVT | 1017 | -12.6% | 8.2 | +0.00183 | -1.82 | yes | n/a |
| icc-gradient-q60 | lazy-image current rav1e | 1383 | baseline | 12.0 | baseline | baseline | n/a | yes |
| icc-gradient-q60 | avifenc rav1e | 1600 | +15.7% | 12.7 | +0.00218 | -0.09 | n/a | yes |
| icc-gradient-q60 | avifenc AOM | 1959 | +41.6% | 5.1 | -0.00240 | -2.15 | n/a | yes |
| icc-gradient-q60 | avifenc SVT | 1432 | +3.5% | 6.0 | +0.00183 | -1.86 | n/a | yes |

### Build / Package Impact

This PR is benchmark evidence only. Runtime package contents, public API,
native package targets, and production build dependencies are unchanged for
macOS, Linux, and Windows. `avifenc` is an operator-provided benchmark CLI, not
a linked lazy-image dependency. Any production backend switch still needs a
separate package-size, CI-build-time, and cross-platform native build impact
report.

### Decision

Do not change the production AVIF default from the current rav1e-backed path in
this PR. libavif v1.4.2 with rav1e is the strongest quality-preserving next
prototype because it reduced bytes on photo-like cases and alpha while
improving SSIM, but it regressed the ICC-gradient byte count by 15.7%.

SVT-AV1 is promising as a future speed-oriented profile because it produced the
largest byte reductions on photo-like cases, but it also reduced objective
quality and still needs matched-quality tuning plus package/build impact
evidence. AOM/libaom is not recommended as the default from this snapshot
because it was not consistently smaller and showed larger regressions on the
generated alpha and ICC cases.
