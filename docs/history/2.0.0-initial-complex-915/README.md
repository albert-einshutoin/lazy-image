# macOS initial-complex resolution — #915

This packet follows #913 / PR #914. Their ten-pair macOS first-complex result
remains INCONCLUSIVE, with interval [0.9825377, 1.2747476]. Those samples,
historical warnings, failed runs and the prior BLOCK publication handoff are
immutable. This experiment does not pool them with new samples.

`protocol.json` was registered before new image measurements. The specialist
identified missing identity and cross-arm guards during the first diagnostic
batch. That interrupted batch is retained as rejected evidence and contributes
to no acceptance result. `protocol-amendment.json` registers the guard repairs
before the replacement batch. It preserves counts, timers, thresholds and
quality requirements.

## Reproduction and interpretation

Use two isolated checkouts at the exact A/B revisions in the protocol. Install
with `npm ci` and use `npm run build`, Rust 1.88.0, SDK 26.5 and distinct Cargo
targets. Keep temp files, npm cache, Cargo targets and outputs on the external
volume. The amendment pins the resulting two macOS bindings. A different build
requires a new preregistered binding amendment, never an automatic substitution.

Run the following from the repository, with `UV_THREADPOOL_SIZE=4`, the pinned
toolchain/SDK, and a **new** output directory for each command:

```sh
node docs/history/2.0.0-initial-complex-915/measure.cjs diagnose "$A" "$B" "$DIAG"
node docs/history/2.0.0-initial-complex-915/analyze.cjs "$DIAG"
node docs/history/2.0.0-initial-complex-915/measure.cjs native "$A" "$B" "$NATIVE" "$DIAG/contracts.json"
node docs/history/2.0.0-initial-complex-915/analyze.cjs "$NATIVE"
node docs/history/2.0.0-initial-complex-915/measure.cjs confirm "$A" "$B" "$CONFIRM" "$DIAG/contracts.json"
node docs/history/2.0.0-initial-complex-915/analyze.cjs "$CONFIRM"
```

Diagnosis uses four independent pairs per arm: original suite / isolated complex
crossed with passive / observed execution. The native control uses another four
pairs and the same `process_and_encode` path through `toBufferWithMetrics`.
Native-control outputs and all full-suite quality axes must equal the ordinary
contract. It is diagnosis only. Observed/passive contrasts quantify perturbation;
neither observed times nor isolated times enter performance acceptance.

Ordinary confirmation uses twenty independent fresh-process pairs, alternating
AB/BA, with one first suite and three subsequent warm suites per process. It
uses no GC profiler, metrics API override or event-loop observer. The original
JPEG → WebP → AVIF → complex sequence, including interleaved quality computation
and output evidence, is unchanged. Each operation's original wall timer is
unchanged. First means first suite after module/fixture/reference preparation,
not OS-cold I/O or process startup. Warm is the median of three per-process
measurements. JPEG, AVIF and complex first/warm upper bootstrap bounds must all
be ≤1.15; exact outputs/metadata/quality must also match. WebP and sharp remain
controls. The historical baseline and 15% time / 5% size thresholds are unchanged.
Only an INCONCLUSIVE twenty-pair result permits one further twenty-pair batch;
remaining uncertainty cannot complete the Goal.

The seeded percentile bootstrap is exploratory uncertainty for the paired
median, not a population latency guarantee. The sample unit is a process pair;
warm runs, formats and encoded output rows are not independent samples. Full
raw timings, ratios, min/max/MAD and arm contrasts are retained in the packet.

## Causal limits and final delivery

The observed original suite has more JS heap/allocation pressure than isolated
complex, but the slowest traced first-complex operations have zero GC-duration
overlap. GC pauses alone do not explain the variation. Native-control wall time
closely follows native total; its small remainder combines admission, queue,
API, JS scheduling and observation overhead and cannot identify any one of them.
CPU/context-switch counts aggregate all process threads. Host scheduling, core
residency, JIT and historical host contention are not uniquely identified.
Neither a forced GC nor a pipeline warmup is introduced to obtain a pass.

There is no established codec/core defect requiring a speculative product
change. The implementation hardens measurement identity and contract checks,
and reuses the existing statistics for the preregistered larger confirmation.
The final controlled result and causal observations are in
`controlled-summary.json`; byte/SHA bindings and all raw evidence references
are in `evidence-index.json`.

The twenty-pair ordinary confirmation passed without the conditional extension:

| Target | First median B/A (95% interval) | Warm median B/A (95% interval) |
| --- | --- | --- |
| JPEG | 1.081 [0.986, 1.139] | 0.993 [0.957, 1.014] |
| AVIF | 0.985 [0.979, 0.998] | 0.984 [0.979, 0.990] |
| Complex | 1.000 [0.988, 1.031] | 1.000 [0.982, 1.012] |

Every sampled output hash/metadata and full-suite quality contract matched.
All 2,176 timed-output rows from 80 accepted processes are retained; only the
twenty ordinary process pairs enter acceptance. The maximum first-complex
paired ratio is **3.096**, also retained. Median PASS does not guarantee tail
latency or eliminate individual host outliers. The old ten-pair INCONCLUSIVE
result is not relabeled PASS.

These measurements execute A = a0db8d7f and B = eafc2dfc with an identical
measurement overlay. Final delivery must separately bind the unchanged product
source/build projection to the reviewed merge SHA; they are not fresh paired
executions of a final candidate tarball. The final external publication handoff
must bind that exact SHA to complete hosted CI, security, release benchmark,
eight non-tag candidate tarballs, twelve native smoke reports and explicit
Wasm evidence or identical-tarball reuse. This source packet alone does not
establish that final validation or authorize tag creation/publication.
