# Roadmap / 開発の優先順位

Direction: make verified web-image artifact preparation easy to adopt in an
existing upload or build pipeline. [Product direction](./PROJECT_PHILOSOPHY.md)
owns positioning and non-goals; this page owns investment order, not release status.

## Available foundations in this checkout

The Node API includes the artifact compiler, CLI, responsive/placeholder helpers,
file-based output, resource controls and structured errors. Wasm provides a separate
upload-preflight API. See [API](./API.md) and [Wasm API](./WASM_PACKAGE_API.md).
Availability here does not establish npm publication or validation on every platform.

## Priorities and completion evidence

| Order | Outcome | Evidence needed before claiming completion |
|---|---|---|
| 1 | An upload/build developer can adopt the compiler | First artifact set, policy explanation, failure handling and application-owned delivery responsibilities are clear; record user trial findings |
| 2 | Artifact correctness and resource costs are assessable | Normal-input acceptance and strict budgets, separate hostile rejection, alpha/ICC boundaries, scoped timing/RSS and retained failed results using existing runners |
| 3 | Codec improvements help the target workload | Shared input/quality constraints, exact encoder settings, time/size trade-offs and unmet targets; compare compiler and codec scope separately |
| 4 | Operational integration is easier | Improve the specific integration friction observed in real upload/build jobs; avoid speculative adapters |
| 5 | Wasm preflight has demonstrated runtime value | Browser/Edge bundle cost, latency, budget outcomes and metadata behavior on named runtimes |

Use [benchmark operations](./BENCHMARK_OPERATIONS.md) for measurement and
[release procedure](./RELEASE.md) for release gates. These priorities do not
schedule implementation, close issues or authorize release work.

## Scope control

HTTP transformation servers, CDN/storage hosting, asset-management UIs, rich
editing, animation, full Wasm parity and true streaming are not prerequisites for
the primary workflow. Reconsider them only with a concrete user need and a bounded
proposal. Encoder changes should follow evidence of user-visible quality or cost
improvement, rather than becoming an independent feature race.

## 日本語要約

主対象は、自分の upload/build pipeline で公開用画像一式を作りたい開発者。
優先順は「導入できる → 正しさとコストを確認できる → codec を改善する →
実際の運用上の摩擦を減らす → Wasm の価値を実測する」。競合の機能数を追うより、
公開前処理を省ける価値を確かめる。リリース済みかどうかは、このロードマップでは判断しない。
