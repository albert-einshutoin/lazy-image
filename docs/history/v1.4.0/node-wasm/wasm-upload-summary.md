# Wasm Upload Benchmark Summary

Generated: 2026-09-25T12:56:49.778Z

Published package: 1.4.0; measuring code revision: 4c356dcd7fc90a7917dfa378f2480e07ccab0356.
Command: `node test/benchmarks/wasm-upload-comparison.bench.js --runtime node --version 1.4.0`. Node details and output hashes: `artifacts/benchmark/wasm-published-evidence.json`.
Edge isolate remains unmeasured. Optional competitor rows are not performance results.
Rows marked `unavailable` document optional competitor baselines that are not installed or cannot execute in the current Node-local harness.

| Scenario | Runtime | Package | Type | Status | Browser assets raw | Browser assets gzip | Package dir gzip | Instantiate | First encode | API total | First caller | Warm median | Bytes out | Target hit | Quality | SSIM | PSNR | Metadata stripped | Memory delta | Notes |
|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---|---:|---:|---:|---|---:|---|
| large-photo-upload-webp | node-native | @alberteinshutoin/lazy-image | native-reference | ok | n/a | n/a | n/a | 11.9 | 3889.4 | 3901.4 | n/a | n/a | 330.1 KB | yes | 86 | 0.9960 | 36.87 | n/a | 0 B | Native Node reference for future Wasm/browser comparisons. |
| large-photo-upload-webp | node-wasm | @alberteinshutoin/lazy-image-wasm@1.4.0 | published-package | ok | n/a | n/a | n/a | 9.2 | 318.2 | 3846.6 | n/a | 2688.5 | 321.0 KB | yes | 86 | n/a | n/a | n/a | n/a | Published npm; inspected output cc80ef7b1bc8488ba195f9ad50e17af75c46dac8fdbb8f13f8f5b06644462eec; full conditions in wasm-published-evidence.json |
| large-photo-upload-webp | node-wasm | jSquash | wasm-codec | not-run | n/a | n/a | 537.5 KB | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | jSquash is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims. |
| large-photo-upload-webp | node-wasm | Squoosh | wasm-codec | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Optional Squoosh package is not installed in this workspace. |
| large-photo-upload-webp | node-wasm | browser-image-compression | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here. |
| large-photo-upload-webp | node-wasm | Compressor.js | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Compressor.js requires DOM Canvas APIs and is not installed here. |
| large-png-upload-jpeg | node-native | @alberteinshutoin/lazy-image | native-reference | ok | n/a | n/a | n/a | 0.0 | 536.9 | 536.9 | n/a | n/a | 164.1 KB | yes | 90 | 0.9961 | 34.39 | n/a | 39.3 MB | Native Node reference for future Wasm/browser comparisons. |
| large-png-upload-jpeg | node-wasm | @alberteinshutoin/lazy-image-wasm@1.4.0 | published-package | ok | n/a | n/a | n/a | 9.2 | 151.0 | 2389.6 | n/a | 2346.9 | 420.0 KB | yes | 88 | n/a | n/a | n/a | n/a | Published npm; inspected output 301fd3e0b8cbfb2d8c6b004efb54243fa7f5d9186a3f0bab569c5ff26d58b67a; full conditions in wasm-published-evidence.json |
| large-png-upload-jpeg | node-wasm | jSquash | wasm-codec | not-run | n/a | n/a | 537.5 KB | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | jSquash is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims. |
| large-png-upload-jpeg | node-wasm | Squoosh | wasm-codec | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Optional Squoosh package is not installed in this workspace. |
| large-png-upload-jpeg | node-wasm | browser-image-compression | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here. |
| large-png-upload-jpeg | node-wasm | Compressor.js | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Compressor.js requires DOM Canvas APIs and is not installed here. |

Metadata-only case (`metadata-budget`; separate input from the two upload scenarios):
Raw evidence: `artifacts/benchmark/wasm-published-evidence.json`; input SHA-256: 29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70.
| Runtime | Input EXIF | Input GPS | Input XMP | Input ICC | Output EXIF | Output XMP | Output ICC | Removed | Output SHA-256 |
|---|---|---|---|---|---|---|---|---|---|
| node-wasm | true | true | true | true | false | false | false | true | 3d4de1967d2db5ff428ef4ce895aacc5d5c35e8c0d271043bdcb44f626b12ba0 |
