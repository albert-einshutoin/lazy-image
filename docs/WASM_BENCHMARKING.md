# Published Wasm upload measurement

This benchmark measures the **published npm package**, not the Wasm source in
the checkout. The API and `wasmModules` contract are in
[WASM_PACKAGE_API.md](./WASM_PACKAGE_API.md). The native row is a separate Node
reference; it is not browser evidence. The v1.3.1 snapshot and its limits are
in [WASM_1.3.1_VERIFICATION.md](./history/WASM_1.3.1_VERIFICATION.md).

## Reproduce

Use Node.js 22 or 24, npm, a native binding for the checkout's platform (for
the optional native reference row), and Google Chrome. The following command
installs the exact published version and esbuild 0.25.10 from
`https://registry.npmjs.org/` into a new OS temporary directory. It executes
the installed package in Node, bundles that same install for Chrome, starts a
loopback HTTP server, opens a fresh headless Chrome profile, and waits for the
browser's posted result. It exits nonzero if either required runtime fails.
The npm script's default version is the checkout's `package.json` version;
pass `--version` for a publication record so the target is explicit.

```bash
npm run test:bench:wasm -- --version 1.3.1
```

On systems where Chrome is elsewhere, pass
`--chrome /absolute/path/to/chrome` after the version. To measure only one
runtime, use:

```bash
node test/benchmarks/wasm-upload-comparison.bench.js --runtime node --version 1.3.1
node test/benchmarks/wasm-upload-comparison.bench.js --runtime browser --version 1.3.1
```

The `--runtime edge` command currently exits nonzero: an Edge isolate is not
measured. The `all` selector means the current required Node + browser pair,
not Edge.

Generated artifacts are in `artifacts/benchmark/`:

- `wasm-published-evidence.json`: source commit, exact versions, registry URLs
  and integrities, import/bundle provenance, fixture hashes and policies,
  per-run metrics, output checksums, HTTP asset requests, and verdict.
- `wasm-upload-summary.json` and `.md`: native reference, published `node-wasm`,
  published `browser-worker`, and clearly marked optional competitor rows.
- `wasm-node-*` and `wasm-browser-*`: actual first-run outputs, including the
  unmet best-effort output. They are independently decoded with sharp and
  checked for format, dimensions, bytes, budget result, and metadata.

`artifacts/` is gitignored. Save the JSON, console log, and images together for
a durable review; the snapshot above does so. The report's `sourceSha` is the
HEAD revision, while `sourceDirty` and `sourceFilesSha256` identify edited code
when a development run precedes commit. The temporary install is removed when
the run ends. `publishedVersion`, `resolved`, and `integrity`
identify the measured package and codecs. The installed `browser.js` import
path and esbuild metafile package inputs must point inside the temporary npm
install, never the workspace.

To reproduce a **summary correction without rerunning image processing**, use
the saved raw evidence and the saved pre-correction summary:

```bash
node test/benchmarks/wasm-upload-comparison.bench.js --reaggregate docs/history/wasm-1.3.1/wasm-published-evidence.json --previous-summary docs/history/wasm-1.3.1/wasm-upload-summary-original.json
```

This writes corrected JSON and Markdown under `artifacts/benchmark/`. The
original `generatedAt`/`sourceSha` identify the measured run; `aggregation`
records the later aggregation time, code commit, and both input hashes. The
saved correction is in [wasm-upload-summary.json](./history/wasm-1.3.1/wasm-upload-summary.json)
and [wasm-upload-summary.md](./history/wasm-1.3.1/wasm-upload-summary.md).

## Scope of each number

The cases are JPEG→WebP and PNG→JPEG with resize and reachable byte budgets,
plus an EXIF/GPS/XMP/ICC-bearing JPEG for metadata and the 10-byte impossible
budget. The dedicated input must first be confirmed to contain all four items.
The normal two inputs contain none of them, so their summary rows report
`metadataStripped: null`; the separate `metadataVerification` section maps
the dedicated input's present items to absent output items. The output
must be decoded and inspected; `metrics.metadataStripped` only reflects the
requested option. An impossible `best-effort` result is a valid image but a
**budget miss**. `strict` must reject with `E502` and is counted separately
from image-conversion success. A missing required runtime is FAIL, while
optional competitor packages may be `unavailable` or `not-run` with reasons.

The Node measurement uses an `ImageData` shim and explicit codec Wasm bytes;
it is not a browser or Edge result. The browser uses native `ImageData` in a
real `DedicatedWorkerGlobalScope`. A page sends an `ArrayBuffer` to the Worker
and receives the output bytes. The Worker fetches five codec Wasm files over
HTTP and passes them through the public `wasmModules` option. This is explicit
injection, not ordinary implicit asset resolution. All JS and Wasm responses
have `Cache-Control: no-store`; each case creates a new Worker, and its two
warm samples reuse that same Worker. The input is fetched before the measured
Worker-start interval. `coldFromBeforeWorkerMs` spans Worker creation through
the first returned image. `warmMs` records two calls and `warmMedianMs` is
their median. These are small-sample observations, not performance thresholds.

Internal `totalMs` starts inside `optimizeUpload`, after optimizer creation.
`firstEncodeMs` is the first encode attempt in the budget search, not the
whole encode/search cost. `instantiateMs` times `initializeCodecs` only; the
Worker's `assetFetchMs` separately times the five HTTP fetches. Codec startup
and browser peak memory cannot be isolated reliably with this helper, so no
memory or standalone initialization claim is made.

The delivery table lists entry JS, Worker JS, no additional chunks, and all
five Wasm files that the chosen explicit-injection setup fetches. `rawBytes`
and `gzipBytes` are file sizes. `firstCaseTransferredBodyBytes` is the actual
compressed response-body total for those assets in the first case; it excludes
HTTP headers and the input image. `totalRunTransferredBodyBytes` includes
repeat Worker loads across all three cases. `packageDirectoryBytes` is the
installed package folder size and is not a browser transfer estimate. Browser
delivery bytes belong only to `browser-worker` rows; `node-wasm` reports them
as not applicable even when both runtimes are selected.

Optional jSquash, Squoosh, browser-image-compression, and Compressor.js rows
remain diagnostics only. Do not claim a competitive win, bundle superiority,
or Edge cold-start behavior from their missing adapters. Native production
claims remain governed by [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).
