# fast_image_resize 6.1.0 comparison for PR #789

The baseline is main `3ffc865739268ae7bd8afa4c0a6882c67a640f12` with fast_image_resize 6.0.0. The candidate updates the root and Fuzz lockfiles to 6.1.0 while retaining oxipng 10.1.1. Root bytemuck changes from 1.24.0 to 1.25.2 to satisfy FIR's 1.25 requirement; Fuzz already has bytemuck 1.25.1. The package feature selection stays `default-features = false, features = ["rayon"]`.

The crates.io source archive is identified by the Cargo.lock checksum `e9c50201dc184ba6553da1695aac20a042efffbe2d84542cee31917c86c3ab1e`. Its Cargo.toml declares `MIT OR Apache-2.0`; the extracted crate includes both license files. SHA-256: `LICENSE-MIT` `59c7bec3a007a291de2369c504545cb61b5f4d7ac9893e50d523f2aff4fcc7c6`, `LICENSE-APACHE` `5caa63b85f1c4683ed495f09991128fd777fda36066e2fcdf0f8533212776ed1`.

The upstream 6.1.0 change relevant to this project is the x86 AVX2 vertical convolution improvement for u8 pixels. This project uses U8x3/U8x4 and Convolution(Lanczos3). Its u16 improvement and one-thread Rayon/Nearest deadlock fix are outside these exercised paths. See the [upstream changelog](https://github.com/Cykooz/fast_image_resize/blob/v6.1.0/CHANGELOG.md), [dependency definition](https://github.com/Cykooz/fast_image_resize/blob/v6.1.0/Cargo.toml), and the project's [resize path](../../../src/engine/resize.rs).

## Method

`compare.cjs` runs four fixed cases: RGB photo with inside fit, 320×240 UI source resized to odd width 157 with fill, RGBA transparency boundaries with cover, and generated fully opaque 1200×900 RGBA with fill. The input SHA-256 values, dimensions, options, output PNG hashes and sizes, and alpha counts are in each `results.json`. The script uses `ImageEngine.from(input).resize(options).toBufferWithMetrics('png')` and records one cold and five sequential warm calls by default; `RESIZE_WARM_COUNT` controls the repeat count. The external wall time includes decode, resize, and PNG encode; `opsMs` is the native operations metric and is reported separately. `RAYON_NUM_THREADS=4` in both versions of each environment.

Output PNGs are decoded independently with sharp 0.35.4 to `toColourspace('srgb').ensureAlpha().raw()` 8-bit RGBA. `comparison.json` records dimensions, full pixel SHA-256, alpha counts, changed-pixel count, and maximum channel delta. Because oxipng remains 10.1.1 in both builds, any output difference can be investigated as a resize change. The added Rust test calls the FIR primary function directly for U8x3, partially transparent U8x4, and 1.08 MP opaque U8x4; API success alone could otherwise hide an image-crate fallback.

Targeted checks on the 6.1.0 candidate passed: the existing `engine::resize::tests` (28 before adding the new direct FIR test), the new direct FIR test separately, `node test/integration/format-matrix.test.js` (21), `node test/integration/golden.test.js` (6), and `node test/integration/edge-cases.test.js` (32). These exercise existing fit, invalid-dimension, alpha, and golden-output cases without changing expected files. Both versions built with Rust 1.88.0 in release mode. The x86 paired run also repeated the direct FIR test with `--release`.

For a same-host replay, build each revision with Rust 1.88.0 in release mode, then run the script once per checkout:

On the measured Mac, `SDKROOT=/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX26.5.sdk` was set for both builds and Rust tests; the installed Command Line Tools SDK otherwise fails in TAPI before compiling project code.

```sh
RAYON_NUM_THREADS=4 RUSTUP_TOOLCHAIN=1.88.0 \
  RESIZE_ROOT="$PWD" RESIZE_OUT_DIR=/tmp/resize-baseline \
  RESIZE_LABEL=baseline RESIZE_REVISION="$(git rev-parse HEAD)" \
  node /path/to/compare.cjs run

RESIZE_ROOT=/path/to/candidate-checkout \
  node /path/to/compare.cjs compare /tmp/resize-baseline /tmp/resize-candidate
```

Run the first command with a different output directory and label for the candidate. The script asserts that the expected local N-API binding is loaded. It emits PNGs and JSON; only JSON is kept here for the local comparison. The paired x86 GitHub Actions artifact retains the PNGs for byte-level follow-up.

## macOS arm64 result

Apple M4, Node 24.2.0, Rust 1.88.0, macOS 26.5 SDK, sharp 0.35.4. The baseline binding SHA-256 is `17b13dc9329312af84c7e0f406888f35fcca84589ccb359929ddb19c2c39e6a9`; candidate binding SHA-256 is `1cf60e149d07e4f9d2f83677c4a658e00e26c6b5f54671366098aa6fded5ab00`. Both builds used the same release command and CPU. See [the first pair](macos-arm64/comparison.json) and [the reversed-order repeat](macos-arm64/comparison-repeat.json). All four cases have identical output dimensions, decoded pixels, alpha counts, PNG bytes and hashes. No material API wall-time regression is evident across the two small five-warm-call samples; changes in the sub-millisecond `opsMs` are within the variation seen across repeats.

Warm public API wall-time medians (6.0.0 → 6.1.0; absolute and percent change) were:

| Case | First pair | Reversed-order repeat |
| --- | --- | --- |
| RGB photo / inside | 24.06 → 21.35 ms (−2.71, −11.3%) | 21.21 → 22.04 ms (+0.83, +3.9%) |
| UI / odd fill | 15.69 → 15.64 ms (−0.06, −0.4%) | 15.40 → 16.10 ms (+0.70, +4.6%) |
| Alpha / cover | 25.78 → 24.68 ms (−1.10, −4.3%) | 24.02 → 24.48 ms (+0.45, +1.9%) |
| Opaque RGBA / fill | 79.87 → 80.70 ms (+0.82, +1.0%) | 79.70 → 80.62 ms (+0.92, +1.2%) |

Each linked JSON also contains all five warm values and their min/max for both versions, cold time, and separate native `opsMs`, `encodeMs`, and `totalMs`. The macOS ARM source code chooses NEON on supported targets, but this Mac comparison did not probe the selected FIR extension at runtime; the environment is verified as arm64 and the native build succeeded.

## Linux x86_64 AVX2 result

The paired [GitHub Actions run](https://github.com/albert-einshutoin/lazy-image/actions/runs/36381737064) and [third run](https://github.com/albert-einshutoin/lazy-image/actions/runs/36384082205) used one Ubuntu 24.04 runner per attempt, AMD EPYC 7763 with four vCPUs, Node 22.23.2, Rust 1.88.0, sharp 0.35.4, and Rayon four threads. Saved [CPU information](linux-x64-avx2/attempt-1/cpu.txt) includes the AVX2 flag; a release-built `Resizer::new().cpu_extensions()` probe printed `FIR_CPU_EXTENSIONS=Avx2` before each measured build. In attempt 1, the baseline and candidate release bindings had SHA-256 `255ce74e317951021864ef09e2480a2638704b372fc93138c2dee8fd6f07efc2` and `35f1d9ab913b031101a6245a9d39df5dba11a5f9eae028da0de749d4782f82d0`; the per-attempt JSON records each rebuilt binding's hash. The direct FIR primary-path test passed in all three attempts.

All three attempts found exact agreement for all four output dimensions, decoded RGBA pixels, alpha counts, PNG bytes and hashes ([attempt 1](linux-x64-avx2/attempt-1/comparison.json), [attempt 2](linux-x64-avx2/attempt-2/comparison.json), [attempt 3](linux-x64-avx2/attempt-3/comparison.json)). Warm public API wall-time medians (6.0.0 → 6.1.0; absolute and percent change) were:

| Case | Attempt 1, five calls | Attempt 2, five calls | Attempt 3, candidate first, 15 calls |
| --- | --- | --- | --- |
| RGB photo / inside | 47.20 → 43.06 ms (−4.13, −8.8%) | 45.95 → 50.88 ms (+4.92, +10.7%) | 41.88 → 43.94 ms (+2.06, +4.9%) |
| UI / odd fill | 30.33 → 29.30 ms (−1.03, −3.4%) | 30.99 → 38.32 ms (+7.33, +23.7%) | 28.68 → 29.31 ms (+0.62, +2.2%) |
| Alpha / cover | 51.33 → 46.23 ms (−5.11, −10.0%) | 65.26 → 55.79 ms (−9.47, −14.5%) | 55.30 → 50.67 ms (−4.63, −8.4%) |
| Opaque RGBA / fill | 163.32 → 163.45 ms (+0.13, +0.1%) | 189.38 → 185.81 ms (−3.58, −1.9%) | 162.04 → 162.91 ms (+0.87, +0.5%) |

Attempt 2's apparent UI slowdown was mostly PNG encode (30.38 → 37.67 ms), while native `opsMs` changed only 0.292 → 0.372 ms. Its sign differed from attempt 1 and the longer reversed-order attempt 3, whose native UI `opsMs` changed 0.312 → 0.319 ms. The linked JSON retains cold calls, every warm value, min/max, and the native and encode components. These data support output compatibility and show no repeatable major regression in the measured public API cases. They do not establish a general throughput improvement: PNG encode dominates these small cases, and the runners vary between attempts. The upstream AVX2 improvement is relevant to the selected u8 path, but its isolated speed effect is below the reliable resolution of this API measurement.

The u16 native improvement and one-thread Rayon/Nearest deadlock repair in the upstream release were not exercised by this project comparison; the application path here uses U8x3/U8x4 Convolution(Lanczos3) and four Rayon threads. This evaluation is an unpublished dependency adoption check, separate from npm 1.4.1 and the held oxipng #833 candidate.
