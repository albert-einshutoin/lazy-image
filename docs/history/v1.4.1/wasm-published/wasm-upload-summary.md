# Wasm Upload Benchmark Summary

Generated: 2026-09-26T15:46:15.334Z

Published package: 1.4.1; measuring code revision: 3ab052ebc223dcf096e2ed21498e964db2035200.
Command: `node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --version 1.4.1 --browser-load default`. Browser details and output hashes: `artifacts/benchmark/wasm-published-evidence.json`.
Browser load mode: default.
Edge isolate remains unmeasured. Optional competitor rows are not performance results.
Rows marked `unavailable` document optional competitor baselines that are not installed or cannot execute in the current Node-local harness.

| Scenario | Runtime | Package | Type | Status | Browser assets raw | Browser assets gzip | Package dir gzip | Instantiate | First encode | API total | First caller | Warm median | Bytes out | Target hit | Quality | SSIM | PSNR | Metadata stripped | Memory delta | Notes |
|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---:|---|
| large-photo-upload-webp | node-native | @alberteinshutoin/lazy-image | native-reference | ok | n/a | n/a | n/a | 25.5 | 3260.0 | 3285.5 | n/a | n/a | 330.1 KB | yes | 86 | 0.9960 | 36.87 | n/a | 34.1 MB | Native Node reference for future Wasm/browser comparisons. |
| large-photo-upload-webp | browser-worker | @alberteinshutoin/lazy-image-wasm@1.4.1 | published-package | ok | 1.3 MB | 410.0 KB | n/a | 0.0 | 195.6 | 2853.0 | 2864.4 | 1732.5 | 321.0 KB | yes | 86 | n/a | n/a | n/a | n/a | Published npm; published Worker helper and codec default loader; no wasmModules; inspected output cc80ef7b1bc8488ba195f9ad50e17af75c46dac8fdbb8f13f8f5b06644462eec; full conditions in wasm-published-evidence.json |
| large-photo-upload-webp | browser-worker | jSquash | wasm-codec | not-run | n/a | n/a | 537.5 KB | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | jSquash is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims. |
| large-photo-upload-webp | browser-worker | Squoosh | wasm-codec | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Optional Squoosh package is not installed in this workspace. |
| large-photo-upload-webp | browser-worker | browser-image-compression | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here. |
| large-photo-upload-webp | browser-worker | Compressor.js | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Compressor.js requires DOM Canvas APIs and is not installed here. |
| large-png-upload-jpeg | node-native | @alberteinshutoin/lazy-image | native-reference | ok | n/a | n/a | n/a | 0.0 | 494.8 | 494.8 | n/a | n/a | 164.1 KB | yes | 90 | 0.9961 | 34.39 | n/a | 40.1 MB | Native Node reference for future Wasm/browser comparisons. |
| large-png-upload-jpeg | browser-worker | @alberteinshutoin/lazy-image-wasm@1.4.1 | published-package | ok | 1.3 MB | 410.0 KB | n/a | 0.0 | 127.5 | 2661.0 | 2672.5 | 1559.1 | 420.0 KB | yes | 88 | n/a | n/a | n/a | n/a | Published npm; published Worker helper and codec default loader; no wasmModules; inspected output 301fd3e0b8cbfb2d8c6b004efb54243fa7f5d9186a3f0bab569c5ff26d58b67a; full conditions in wasm-published-evidence.json |
| large-png-upload-jpeg | browser-worker | jSquash | wasm-codec | not-run | n/a | n/a | 537.5 KB | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | jSquash is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims. |
| large-png-upload-jpeg | browser-worker | Squoosh | wasm-codec | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Optional Squoosh package is not installed in this workspace. |
| large-png-upload-jpeg | browser-worker | browser-image-compression | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here. |
| large-png-upload-jpeg | browser-worker | Compressor.js | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Compressor.js requires DOM Canvas APIs and is not installed here. |

Metadata-only case (`metadata-budget`; separate input from the two upload scenarios):
Raw evidence: `artifacts/benchmark/wasm-published-evidence.json`; input SHA-256: 29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70.
| Runtime | Input EXIF | Input GPS | Input XMP | Input ICC | Output EXIF | Output XMP | Output ICC | Removed | Output SHA-256 |
|---|---|---|---|---|---|---|---|---|---|
| browser-worker | true | true | true | true | false | false | false | true | 3d4de1967d2db5ff428ef4ce895aacc5d5c35e8c0d271043bdcb44f626b12ba0 |
