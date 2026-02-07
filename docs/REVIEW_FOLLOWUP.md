# Review follow-up (publication readiness)

Checklist from code review: *「120%の自信で世界に公開できるか」* — items to address before marketing lazy-image as "smaller than sharp" with full confidence.

## Done in this repo

- [x] **tsconfig.json** with `strict: true` for `test:types` (type-check effectiveness).
- [x] **Fuzz limit unit tests**: `#[cfg(feature = "fuzzing")]` tests for `FUZZ_MAX_DIMENSION` / `FUZZ_MAX_PIXELS`; large-dimension tests gated with `#[cfg(not(feature = "fuzzing"))]` so `cargo test --features fuzzing` does not break.

## Pending (repository-wide)

| # | Action | Notes |
|---|--------|--------|
| 1 | Re-measure README benchmark numbers and align with TRUE_BENCHMARKS.md | README (5000x5000) vs TRUE_BENCHMARKS.md: JPEG size, AVIF time, RSS の乖離を解消 |
| 2 | Fill `README_VALUES` in readme-verification.bench.js | 空のままなので README 数値の自動検証が効いていない |
| 3 | Document or fix AVIF resize vs sharp | TRUE_BENCHMARKS: AVIF no-resize -40.9% (win), AVIF resize 800px +9.5% (loss). README の "Smaller than sharp" を条件付きで明記するか改善する |
| 4 | Add benchmark hard gate | sharp-comparison.bench.js に「sharp よりファイルサイズが小さい」アサーションを追加。benchmark-regression.yml の fail-on-alert 検討 |

## References

- README ヘッドライン数値の出典: 再計測して TRUE_BENCHMARKS.md と一致させる。
- AVIF resize で sharp に負けている事実: README に明記するか、品質/速度のトレードオフを改善する。
