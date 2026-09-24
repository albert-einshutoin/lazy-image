# Published Wasm upload measurement

This benchmark measures the **published npm package**, not the Wasm source in
the checkout. The API and `wasmModules` contract are in
[WASM_PACKAGE_API.md](./WASM_PACKAGE_API.md). The native row is a separate Node
reference; it is not browser evidence. The v1.3.1 Node/Chrome snapshot and its
limits are in [WASM_1.3.1_VERIFICATION.md](./history/WASM_1.3.1_VERIFICATION.md);
the separate local workerd measurement is in
[WASM_1.3.1_EDGE_VERIFICATION.md](./history/WASM_1.3.1_EDGE_VERIFICATION.md).

## Reproduce

Use Node.js 22 or 24, npm, a native binding for the checkout's platform (for
the optional native reference row), Google Chrome, and a host supported by
`workerd@1.20260924.1`. The following command installs the exact published
version, esbuild 0.25.10, and workerd from
`https://registry.npmjs.org/` into a new OS temporary directory. It executes
the installed package in Node, bundles that same install for Chrome and local
workerd, and runs all three required runtimes. It exits nonzero if any required
runtime fails or cannot be prepared. The Edge run also fetches the
version-pinned workerd LICENSE from Cloudflare's GitHub repository because the
npm toolchain packages omit that file; it records its hash and the installed
workerd/esbuild executable hashes.
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
node test/benchmarks/wasm-upload-comparison.bench.js --runtime edge --version 1.3.1
```

`all` means Node + Chrome Worker + local workerd; selecting only `node` or
`browser` does not verify Edge. The Edge adapter executes the published
`/edge` entrypoint inside workerd and requires no Chrome. Pass
`--workerd /absolute/path/to/workerd` only when overriding the executable
installed in the isolated directory. A missing workerd executable is BLOCKED;
an isolate startup/module/image failure is FAIL. The focused negative checks
are reproducible with:

```bash
node test/benchmarks/wasm-edge-failure-check.mjs 1.3.1
```

Generated artifacts are in `artifacts/benchmark/`:

- `wasm-published-evidence.json`: source commit, exact versions, registry URLs
  and integrities, import/bundle provenance, fixture hashes and policies,
  per-run metrics, output checksums, browser HTTP asset requests when selected,
  and verdict.
- `wasm-upload-summary.json` and `.md`: native reference, published `node-wasm`,
  `browser-worker`, `edge-isolate` (only for selected runtimes), and clearly
  marked optional competitor rows.
- `wasm-node-*`, `wasm-browser-*`, and `wasm-edge-*`: actual first-run outputs, including the
  unmet best-effort output. They are independently decoded with sharp and
  checked for format, dimensions, bytes, budget result, and metadata.

`artifacts/` is gitignored. Save the JSON, console log, and images together for
a durable review; the snapshot above does so. The report's `sourceSha` is the
HEAD revision, while `sourceDirty` and `sourceFilesSha256` identify edited code
when a development run precedes commit. The temporary install is removed when
the run ends. `publishedVersion`, `resolved`, and `integrity`
identify the measured package and codecs. The installed `browser.js` or
`edge.js` import path and esbuild metafile package inputs must point inside
the temporary npm install, never the workspace.

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
from image-conversion success. A missing workerd executable is BLOCKED; an
isolate that starts but fails a required case is FAIL. Both exit nonzero. Other
missing required runtimes also exit nonzero, while optional competitor
packages may be `unavailable` or `not-run` with reasons.

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
as not applicable even when all runtimes are selected. The Edge asset columns
belong only to `edge-isolate` rows.

The local Edge adapter starts a fresh workerd process for the small probe and
each case. Its `/health` endpoint verifies five real
`WebAssembly.Module` imports and does not create an optimizer or process an
image. The first image request creates one optimizer; two warm requests reuse
that same isolate and optimizer. Node's `performance.now()` measures from
process start to receipt of the first image, the first request round trip, and
two warm round trips. The Edge `/clock` diagnostic runs only afterward;
workerd's local `performance.now()` advanced during the CPU loop in measured
cases, but any failed diagnostic is recorded as unavailable. Internal API
metrics have a different scope and do not replace caller timing. The adapter
does not install an ImageData/DOM shim or replace codecs; it bundles the
published `/edge` package and passes workerd's statically imported Wasm
modules through `wasmModules`. The listed JS and five Wasm files are static
deployment raw/gzip sizes, not HTTP transfer bytes. No production cold-start,
billed CPU, peak-memory, or all-Edge-runtime claim follows from local workerd.

Optional jSquash, Squoosh, browser-image-compression, and Compressor.js rows
remain diagnostics only. Do not claim a competitive win, bundle superiority,
or Edge cold-start behavior from their missing adapters. Native production
claims remain governed by [TRUE_BENCHMARKS.md](./TRUE_BENCHMARKS.md).
