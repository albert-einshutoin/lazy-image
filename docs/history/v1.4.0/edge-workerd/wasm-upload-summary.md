# Wasm Upload Benchmark Summary

Generated: 2026-09-25T10:56:34.652Z

Published package: 1.4.0; measuring code revision: 9c22cf3cdeeb7aa515b59107a2834e2c14495323.
Command: `node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.4.0`. Edge details and output hashes: `artifacts/benchmark/wasm-published-evidence.json`.
Edge results are local workerd observations; production cold-start and CPU billing are unmeasured. Optional competitor rows are not performance results.
Rows marked `unavailable` document optional competitor baselines that are not installed or cannot execute in the current Node-local harness.

| Scenario | Runtime | Package | Type | Status | Browser assets raw | Browser assets gzip | Edge assets raw | Edge assets gzip | Ready | First request | Package dir gzip | Instantiate | First encode | API total | First caller | Warm median | Bytes out | Target hit | Quality | SSIM | PSNR | Metadata stripped | Memory delta | Notes |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| large-photo-upload-webp | edge-isolate | @alberteinshutoin/lazy-image-wasm@1.4.0 | published-package | ok | n/a | n/a | 1.2 MB | 397.3 KB | 29.4 | 3675.6 | n/a | 4.0 | 285.0 | 3667.0 | 3705.5 | 2619.5 | 321.0 KB | yes | 86 | n/a | n/a | n/a | n/a | Published npm; inspected output cc80ef7b1bc8488ba195f9ad50e17af75c46dac8fdbb8f13f8f5b06644462eec; full conditions in wasm-published-evidence.json |
| large-photo-upload-webp | edge-isolate | jSquash | wasm-codec | not-run | n/a | n/a | n/a | n/a | n/a | n/a | 537.5 KB | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | jSquash is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims. |
| large-photo-upload-webp | edge-isolate | Squoosh | wasm-codec | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Optional Squoosh package is not installed in this workspace. |
| large-photo-upload-webp | edge-isolate | browser-image-compression | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here. |
| large-photo-upload-webp | edge-isolate | Compressor.js | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Compressor.js requires DOM Canvas APIs and is not installed here. |
| large-png-upload-jpeg | edge-isolate | @alberteinshutoin/lazy-image-wasm@1.4.0 | published-package | ok | n/a | n/a | 1.2 MB | 397.3 KB | 30.5 | 3070.3 | n/a | 4.0 | 162.0 | 3062.0 | 3101.6 | 1995.4 | 420.0 KB | yes | 88 | n/a | n/a | n/a | n/a | Published npm; inspected output 301fd3e0b8cbfb2d8c6b004efb54243fa7f5d9186a3f0bab569c5ff26d58b67a; full conditions in wasm-published-evidence.json |
| large-png-upload-jpeg | edge-isolate | jSquash | wasm-codec | not-run | n/a | n/a | n/a | n/a | n/a | n/a | 537.5 KB | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | jSquash is installed, but this first harness records npm package-directory size only; add a browser bundle adapter before making bundle or performance claims. |
| large-png-upload-jpeg | edge-isolate | Squoosh | wasm-codec | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Optional Squoosh package is not installed in this workspace. |
| large-png-upload-jpeg | edge-isolate | browser-image-compression | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | browser-image-compression requires browser File/Canvas/Worker APIs and is not installed here. |
| large-png-upload-jpeg | edge-isolate | Compressor.js | browser-compressor | unavailable | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | Compressor.js requires DOM Canvas APIs and is not installed here. |

Metadata-only case (`metadata-budget`; separate input from the two upload scenarios):
Raw evidence: `artifacts/benchmark/wasm-published-evidence.json`; input SHA-256: 29a8b5ce307f40c8fa546da2e3773341451bcfc60106eb62fb71d8a101dfce70.
| Runtime | Input EXIF | Input GPS | Input XMP | Input ICC | Output EXIF | Output XMP | Output ICC | Removed | Output SHA-256 |
|---|---|---|---|---|---|---|---|---|---|
| edge-isolate | true | true | true | true | false | false | false | true | 3d4de1967d2db5ff428ef4ce895aacc5d5c35e8c0d271043bdcb44f626b12ba0 |
