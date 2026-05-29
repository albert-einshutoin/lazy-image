# Rust Engineering Review — lazy-image

> Branch: `feature/rust-idiom-hardening` | Date: 2026-05-29 | Rust 1.88.0 | Edition 2021

---

## Table of Contents

1. [Architectural Review](#1-architectural-review)
2. [Design Summary](#2-design-summary)
3. [Performance Analysis](#3-performance-analysis)
4. [Rust-Specific Improvements](#4-rust-specific-improvements)
5. [Audit Report](#5-audit-report)
6. [Quality Validation](#6-quality-validation)
7. [Prioritized Refactor Roadmap](#7-prioritized-refactor-roadmap)

---

## 1. Architectural Review

### 1.1 Design Strengths

The codebase is well-structured for a compression-first image engine with NAPI bindings. Notable strengths:

**Ownership model.** `Source` wraps buffers in `Arc` for zero-copy task clones. `Cow<DynamicImage>` through `apply_ops_tracked` avoids pixel copies when no ops are queued. `PreparedEncode.processed: Arc<DynamicImage>` shares the decoded+ops result across binary-search quality iterations without re-decoding. `OnceLock<Option<Arc<Vec<u8>>>>` ensures ICC and EXIF metadata is parsed at most once per engine instance.

**Type-driven error taxonomy.** `LazyImageError` → `ErrorCode` → `ErrorCategory` with `is_recoverable()` derived from `category()` gives callers a stable, parseable interface. `run_with_panic_policy` + `catch_unwind` converts third-party codec panics to `LazyImageError::InternalPanic` before they reach JavaScript.

**Lazy evaluation.** Source data is cloned into each `AsyncTask` so the engine is non-destructive and reusable. The binary-search quality loop in `EncodeTargetBytesTask` calls `prepare_for_encode` exactly once, reusing the decoded image across all candidate qualities.

**Concurrency model.** Global `rayon::ThreadPool` initialized via `OnceLock` (zero per-call overhead after first use). `WeightedSemaphore` with per-waiter `AtomicBool` flag prevents ABA wakeup issues and serializes oversized jobs before they reach the encode path. `BatchFileContext` is `#[derive(Clone, Copy)]` so it crosses rayon workers as a pointer copy.

**Compile-time safety.** `const _: fn() = || { assert_send::<T>() }` on all task types. `#![deny(unsafe_op_in_unsafe_fn)]` in `avif_safe.rs`. Compile-time assertions catch constant invariant violations (e.g., `ZUNE_PNG_MAX_DIM <= MAX_DIMENSION`) before tests run.

### 1.2 Design Weaknesses

**Unsafe discipline gap.** `avif_safe.rs` (30 unsafe occurrences) has zero `// SAFETY:` comments despite the file-level `deny(unsafe_op_in_unsafe_fn)` posture. The alpha-plane write loop at `encoder.rs:723–728` relies on `debug_assert!` bounds checks that are elided in release builds. A `const`-to-`mut` pointer cast at `avif_safe.rs:478` is formally a provenance violation under Stacked Borrows when the source image is `Cow::Borrowed`.

**Duplicate error classification.** `ErrorCode::category()` and `LazyImageError::category()` are two independent 23–24-arm match tables that must be kept in sync manually (`src/error.rs:120–155` and `681–724`). Divergence silently misclassifies errors.

**Incoherent metadata state.** `MetadataPolicy { icc: bool, exif: bool, gps: bool }` allows `gps: true` with `exif: false`, producing silent wrong output (GPS stripped because EXIF is not embedded). The batch path at `tasks/batch.rs:53–54` never reads `strip_gps()` at all — GPS is always stripped in batch mode regardless of policy.

**Panic exposure at the NAPI boundary.** `napi_err()` at `engine.rs:36` calls `.expect()` on a fallible NAPI allocation, which can abort the Node.js process under heap pressure. This helper is used at ~30 call sites.

**Code duplication.** `BatchTask::compute()` and `BatchWithMetricsTask::compute()` share ~80 structurally identical lines with a diverged mutex-poison-handling detail. `TaskContext` assembly is copy-pasted across 7 output methods in `output_ops.rs`.

**Oversized files.** `encoder.rs` (1081 lines), `error.rs` (840), `ops.rs` (831), `resize.rs` (767), `estimate.rs` (759) exceed the 800-line ceiling established in the project coding style. `encoder.rs` mixes codec logic with EXIF/ICC embedding concerns.

### 1.3 Type-System Opportunities

- `Quality` newtype (wrapping `u8`, enforcing 1..=100) would make `with_quality()` safe-by-construction.
- `RotationAngle` enum (`Cw90/Cw180/Cw270`) eliminates the defensive `_ => Err(...)` branch in `apply.rs`.
- `MetadataPolicy` as a sum type (`StripAll | KeepIcc | KeepExif { strip_gps } | KeepAll { strip_gps }`) makes the incoherent GPS-without-EXIF state unrepresentable.
- `#[napi(string_enum)]` on `SanitizePolicy` and `ResizeFitMode` eliminates runtime string parsing and its `_ =>` fallback arms.

### 1.4 Performance Bottlenecks

1. `fir::Resizer` is constructed fresh per resize call, discarding its internal `Vec<u8>` buffers.
2. `std::sync::Mutex` in the bounded-concurrency batch path where `parking_lot` is already a dependency.
3. `sanitize_exif_bytes` unconditionally allocates and copies the full EXIF buffer even on the no-op path.
4. `embed_icc_png` / `embed_icc_webp` call `icc.to_vec()` rather than `Bytes::copy_from_slice(icc)`.
5. `available_parallelism()` syscall executed inside every `encode_avif()` invocation.
6. The criterion bench suite covers only synthetic workloads (Fibonacci, atomic counters, raw `fir::Image`). No benchmarks exist for real JPEG/WebP decode→encode pipelines, making it impossible to detect regressions in the actual hot path.

### 1.5 Major Trade-offs and Risks

| Trade-off | Current decision | Risk |
|-----------|-----------------|------|
| Compression-first (mozjpeg/libavif) | Accepted; slower than sharp | None — stated goal |
| NAPI binary, not a Rust library | Rust API surface is pub(crate) | Low coupling to downstream |
| Panic guard for all codec entry points | `run_with_panic_policy` | `napi_err` `.expect()` bypasses this |
| Per-file rayon dispatch | Good throughput | Batch duplication adds maintenance risk |
| MetadataPolicy as three booleans | Simple NAPI mapping | Active bug: batch GPS always stripped |

---

## 2. Design Summary

Each entry below describes a confirmed change, why it matters, and its expected benefit. Items are grouped by the module they primarily affect.

### 2.1 Error Architecture

**Delegate `LazyImageError::category()` to `ErrorCode::category()`** (`src/error.rs:681–724`).
Remove the 43-line duplicate match and replace with `self.code().category()`. Eliminates the risk of two tables diverging; the compiler enforces exhaustiveness only once.

**Replace `napi_err().expect()` with a safe fallback** (`src/engine/api/engine.rs:36`).
Change to `napi_error_with_code(env, err).unwrap_or_else(|_| napi::Error::from_status(napi::Status::GenericFailure))`. Removes the only remaining panic path that could abort the Node.js process under normal codec operation.

**Replace `String` error types in `FromStr` implementations** (`src/ops.rs:194, 235, 245`).
Use `LazyImageError` as `type Err` for `ResizeFit::FromStr` and the `OutputFormat` parse functions. Eliminates the silent discard at `pipeline_ops.rs:66` (`.map_err(|_| ...)`).

### 2.2 Type System

**Introduce `Quality` newtype** (`src/ops.rs:300–310`).
`Quality(u8)` with `Quality::new` (range-checked) and `Quality::unchecked` (pre-validated hot path). Store in `OutputFormat` variants instead of bare `u8`. Zero-cost at runtime; enforces the 1..=100 invariant at construction.

**Introduce `RotationAngle` enum** (`src/ops.rs:94`).
Replace `Operation::Rotate { degrees: i32 }` with `Operation::Rotate(RotationAngle)` where `RotationAngle` has variants `Cw90/Cw180/Cw270`. Removes the `_ => Err(...)` fallback in `apply.rs:291–293`.

**Model `MetadataPolicy` as a sum type** (`src/engine/metadata.rs:14–21`).
`pub enum MetadataPolicy { StripAll, KeepIcc, KeepExif { strip_gps: bool }, KeepAll { strip_gps: bool } }`. Simultaneously fixes the silent wrong-output bug (GPS-keep-without-EXIF is unrepresentable) and the batch GPS omission (`tasks/batch.rs:53–54`).

**Add `#[napi(string_enum)]` for `SanitizePolicy` and `ResizeFitMode`** (`src/engine/api/types.rs`).
Eliminates `_ =>` fallback arms in `pipeline_ops.rs:209–213` and `64–66`. Consistent with the existing `FirewallPolicy` enum treatment.

### 2.3 Ownership / Allocations

**`sanitize_exif_bytes` → return `Cow<'_, [u8]>`** (`src/engine/encoder.rs:432–524`).
Return `Cow::Borrowed(exif)` on the no-op paths (lines 436, 443, and when neither flag fires). The unconditional `exif.to_vec()` at line 474 moves inside the mutation branches. Eliminates a full EXIF buffer copy (up to 512 KB) on every encode call that preserves EXIF metadata.

**ICC embed: `Bytes::copy_from_slice(icc)` instead of `Bytes::from(icc.to_vec())`** (`src/engine/encoder.rs:562, 634`).
Drops one intermediate `Vec` allocation per PNG/WebP encode that embeds an ICC profile.

**`FirewallConfig`: add `Copy`** (`src/engine/firewall.rs:35–45`).
All fields are `Copy`. Nine explicit `.clone()` calls in `output_ops.rs` and `batch.rs` become implicit copies.

**`format_to_string`: return `&'static str`** (`src/engine/tasks/context.rs:26–39`).
Change `PreparedEncode::input_format` to `Option<&'static str>`. Convert to `String` only at `ProcessingMetrics` population. Eliminates two heap allocations per encode on the metrics path.

### 2.4 Unsafe Code

**Add `// SAFETY:` comments to all `unsafe` blocks in `avif_safe.rs`** (30 occurrences).
Documentation-only; no behavior change. Required for audit traceability and consistent with `encoder.rs:706`.

**Elevate `create_rgb_image` to `pub unsafe fn`** (`src/codecs/avif_safe.rs:439`).
The function stores a raw pointer for FFI use; marking it `unsafe` propagates the invariant contract to callers. Update the single call site in `encoder.rs:705` (already inside an `unsafe` block).

**Fix const-to-mut pointer cast provenance violation** (`src/codecs/avif_safe.rs:478`).
Call `rgba.into_owned()` before extracting the pointer in the `Cow::Borrowed` arm so the raw pointer always has mutable provenance. Eliminates formally UB code on the hot AVIF encode path.

**Harden alpha-plane write bounds from `debug_assert!` to a hard check** (`src/engine/encoder.rs:723, 728`).
Replace `debug_assert!` with an explicit `if alpha_row_bytes < width as usize { return Err(...) }` before the loop. Converts a potential silent OOB write in release builds into a recoverable error.

**Propagate `unsafe_op_in_unsafe_fn` deny crate-wide** (`src/lib.rs`).
Add `#![deny(unsafe_op_in_unsafe_fn)]` to `src/lib.rs` so future `unsafe fn` additions in any module outside `avif_safe.rs` require explicit inner `unsafe` blocks.

### 2.5 Concurrency / Batch

**Align `BatchWithMetricsTask` mutex handling with `BatchTask`** (`src/engine/tasks/batch.rs:421, 427`).
Replace bare `.unwrap()` with `.unwrap_or_else(|p| p.into_inner())`. Replace `Arc::try_unwrap(out).unwrap().into_inner().unwrap()` with the guarded lock+take pattern from `BatchTask:283–288`. Prevents cascade panics on mutex poison.

**Switch bounded-concurrency batch paths from `std::sync::Mutex` to `parking_lot::Mutex`** (`src/engine/tasks/batch.rs:254, 403`).
`parking_lot` is already a dependency. Eliminates dead poison-recovery branches and makes the two bounded paths consistent.

**Cache `available_parallelism()` in a `OnceLock<i32>`** (`src/engine/encoder.rs:738–742`).
Eliminates N syscalls per AVIF batch. Zero behavioral change.

### 2.6 Public API

**Make PNG quality rejection explicit** (`src/ops.rs:261–265`).
Return `Err` with a clear message when `format == "png"` and `quality.is_some()`. The current silent discard is a footgun: `validate_quality` runs (implying acceptance) but the value is thrown away.

**Deprecate or encapsulate `preset()`** (`src/engine/api/pipeline_ops.rs:383–421`).
`preset()` mutates engine state AND returns a passive value for a separate output call, violating command-query separation. Add a prominent deprecation notice pointing callers to the self-contained `encode({ preset: 'thumbnail' })` API.

**Mark `toBufferTargetBytesNative` as `@internal`** (`index.d.ts:105`).
Add `@internal` / `@deprecated` JSDoc tags and change `format: string` to `format: OutputFormat` so it is consistent with all sibling output methods.

### 2.7 Module Structure

**Extract EXIF/ICC embedding from `encoder.rs`** (`src/engine/encoder.rs:268–524`).
Move `ExifEmbeddable`, `embed_exif_*`, `embed_icc_*`, and `sanitize_exif_bytes` to `src/engine/io/exif_embed.rs` (or merge into the existing `src/engine/io/exif.rs` at 433 lines). `encoder.rs` drops from 1081 to ~560 lines.

**Add real pipeline benchmarks to the criterion suite** (`benches/benchmark.rs`).
Add `bench_jpeg_roundtrip`, `bench_webp_encode`, and `bench_resize_pipeline` using `include_bytes!()` fixtures and `criterion::Throughput::Bytes`. Without these, the allocation improvements identified in this review cannot be validated against regressions.

---

## 3. Performance Analysis

### 3.1 Identified Bottlenecks

| # | Location | Issue | Impact |
|---|----------|--------|--------|
| P1 | `src/engine/resize.rs:385` | `fir::Resizer::new()` per resize call, discards 3 internal `Vec<u8>` buffers | Medium — in a 100-file batch with 4k images, ~100 alloc/grow/free cycles for convolution buffers |
| P2 | `src/engine/encoder.rs:432–524` | `sanitize_exif_bytes` unconditional `exif.to_vec()` | Medium — up to 512 KB copy per encode call that preserves EXIF |
| P3 | `src/engine/encoder.rs:562, 634` | `Bytes::from(icc.to_vec())` intermediate `Vec` per PNG/WebP ICC embed | Medium — ICC profiles up to 500+ KB for wide-gamut |
| P4 | `src/engine/encoder.rs:738–742` | `available_parallelism()` syscall per `encode_avif()` | Medium — 100–500 µs per 100-file AVIF batch |
| P5 | `src/engine/tasks/batch.rs:254, 403` | `std::sync::Mutex` in bounded-concurrency path vs `parking_lot` already used elsewhere | Low (lock is uncontended in practice, but introduces dead poison-recovery boilerplate) |
| P6 | `src/engine/tasks/batch.rs:113–116` | Double `to_string_lossy()` per batch file | Low — 1000 redundant scans in a 1000-file batch |
| P7 | `src/engine/encoder.rs:288, 325, 342, 359, 532, 564, 636` | `Vec::new()` without capacity hints in embed/encode paths | Medium — ~21 realloc+memcpy cycles per 2 MB image |
| P8 | `src/engine/pipeline/optimize.rs:16–19` | `ops.to_vec()` for len < 2 allocates a `Vec` immediately discarded for empty pipelines | Low — minor per-call allocation |

### 3.2 Optimization Strategy

**Quick wins (no behavior change, 1–5 lines each):**
- `available_parallelism()` → `OnceLock<i32>` (`encoder.rs:738`)
- `Bytes::from(icc.to_vec())` → `Bytes::copy_from_slice(icc)` (`encoder.rs:562, 634`)
- `Vec::new()` → `Vec::with_capacity(...)` in embed paths (`encoder.rs:288, 325, 342, 359, 532`)
- Double `to_string_lossy()` → single capture + `into_owned()` (`batch.rs:113–116`)

**Medium effort (API/signature changes):**
- `sanitize_exif_bytes` → `Cow<'_, [u8]>` return type (2 callers to update)
- `fir::Resizer` → `thread_local! { static RESIZER: RefCell<fir::Resizer> }` inside `resize_with_source_image`

**Structural (refactor required):**
- Add real pipeline benchmarks before measuring any of the above to establish a baseline

### 3.3 Benchmark Methodology

**Existing suite** (`benches/benchmark.rs`, `benches/memory_semaphore.rs`):

| Group | What it measures | Relevance to hot path |
|-------|-----------------|----------------------|
| `thread_pool` | Pool construction overhead | High (validates OnceLock) |
| `parallel_processing` | `fibonacci()` with rayon | Low (synthetic) |
| `concurrency_levels` | `fibonacci()` at N threads | Low (synthetic) |
| `fir_lanczos` | Raw `fir::Image` resize 4000×3000→1000×750 | Medium (no decode/encode) |
| `memory_semaphore` | std vs parking_lot semaphore, 64 threads | High for semaphore work |

**Gaps to fill:**

```rust
// benches/benchmark.rs — add these three groups

fn bench_jpeg_roundtrip(c: &mut Criterion) {
    let jpeg_bytes = include_bytes!("../test/fixtures/photo_4000x3000.jpg");
    let mut group = c.benchmark_group("jpeg_roundtrip");
    group.throughput(Throughput::Bytes(jpeg_bytes.len() as u64));
    group.bench_function("decode_encode_q80", |b| {
        b.iter(|| {
            let decoded = decode_jpeg_mozjpeg(std::hint::black_box(jpeg_bytes)).unwrap();
            encode_jpeg_with_settings(&decoded, 80, false).unwrap()
        })
    });
    group.finish();
}

fn bench_resize_pipeline(c: &mut Criterion) { /* fast_resize_owned 4000×3000→800×600 */ }
fn bench_webp_encode(c: &mut Criterion)     { /* encode_webp on decoded 4000×3000 RGBA */ }
```

### 3.4 Before/After Benchmarking Plan

1. **Establish baseline** on `main` (or current HEAD before any optimization):
   ```
   cargo bench --no-default-features -- --save-baseline before
   ```
2. **Apply one optimization at a time** on a separate branch.
3. **Compare**:
   ```
   cargo bench --no-default-features -- --baseline before
   ```
4. **Targets to watch** after each optimization:
   - `sanitize_exif_bytes` Cow refactor → `jpeg_roundtrip` mean should decrease for EXIF-preserving paths
   - `fir::Resizer` thread-local → `fir_lanczos` mean should decrease for repeat calls
   - `available_parallelism()` OnceLock → `encode_avif` (once added as a bench target)
5. Run on a quiet machine with CPU frequency pinned (disable Turbo Boost).

> **Note:** "After" numbers require implementation. Criterion's `--baseline` comparison generates per-function Δ% with confidence intervals in `target/criterion/`.

---

## 4. Rust-Specific Improvements

### 4.1 Ownership

| Finding | File:Line | Change |
|---------|-----------|--------|
| `sanitize_exif_bytes` allocates unconditionally | `encoder.rs:474` | Return `Cow<'_, [u8]>`; `Cow::Borrowed` on no-op paths |
| ICC embed intermediate `Vec` | `encoder.rs:562, 634` | `Bytes::copy_from_slice(icc)` |
| `FirewallConfig` cloned 9× but is `Copy`-eligible | `firewall.rs:35–45` | Add `#[derive(Copy)]` |
| `format_to_string` → `String` from `&'static str` | `context.rs:26–39` | Return `&'static str`; convert to `String` only at metrics population |
| `fast_resize` public export copies pixel buffer | `engine.rs:85` | Add doc comment distinguishing from `fast_resize_owned`; do not reduce visibility (used by `tests/edge_cases.rs:9`) |

### 4.2 Type-System

| Finding | File:Line | Change |
|---------|-----------|--------|
| Quality `u8` without construction invariant | `ops.rs:300–310` | `Quality` newtype with `Quality::new` / `Quality::unchecked` |
| `Operation::Rotate { degrees: i32 }` accepts any integer | `ops.rs:94` | `RotationAngle` enum `{ Cw90, Cw180, Cw270 }` |
| `MetadataPolicy` three booleans allow incoherent GPS state | `metadata.rs:14–21` | Sum type `MetadataPolicy` enum |
| `SanitizeOptions::policy: Option<String>` | `api/types.rs:39–41` | `#[napi(string_enum)] SanitizePolicy` enum |
| `ResizeOptions::fit: Option<String>` | `api/types.rs:28–35` | `#[napi(string_enum)] ResizeFitMode` enum |
| `OutputFormat::from_str` shadows `std::str::FromStr` | `ops.rs:235–237` | Rename inherent `from_str` to `parse_format`; do not implement `FromStr` (quality arg incompatible) |

### 4.3 Error Handling

| Finding | File:Line | Change |
|---------|-----------|--------|
| Duplicate `category()` tables | `error.rs:681–724` | Delete `LazyImageError::category()` body; delegate to `self.code().category()` |
| `#[derive(Clone)]` replaces 105-line manual impl | `error.rs:285–390` | Add `#[derive(Clone)]`; delete manual `impl Clone` |
| `napi_err().expect()` can abort Node.js process | `engine.rs:36` | `unwrap_or_else(|_| napi::Error::from_status(GenericFailure))` |
| `String` error type in `FromStr` impls | `ops.rs:194, 235, 245` | `type Err = LazyImageError` |
| `avif_safe.rs` `expect()` in `set_color_properties` / `configure` returns `()` | `avif_safe.rs:104, 279` | Return `Result<(), LazyImageError>` matching peer methods |
| `expect("source always has bytes")` in batch worker | `batch.rs:70` | `.ok_or_else(|| LazyImageError::internal_panic(...))?` |
| `let _ = lock.set(...)` silently discards `OnceLock::set` result | `engine.rs:147, 154` | `.expect("freshly-created OnceLock must accept first set")` or use `get_or_init` |
| `ResizeFit::from_str` / `OutputFormat::from_str` silent discard | `pipeline_ops.rs:66` | Eliminated by adopting `LazyImageError` as `Err` type |

### 4.4 Concurrency

| Finding | File:Line | Change |
|---------|-----------|--------|
| Semaphore slow-path N–1 useless lock acquisitions per `notify_all` | `semaphore.rs:121–135` | `cvar.wait_while(&mut guard, \|_\| !ready_flag.load(Acquire))` |
| `BatchWithMetricsTask` bare `.unwrap()` on `Mutex` | `batch.rs:421, 427` | `.unwrap_or_else(\|p\| p.into_inner())` matching `BatchTask` |
| Work-steal dispatch duplicated between `BatchTask` and `BatchWithMetricsTask` | `batch.rs:250–293, 398–430` | Extract `fn work_steal_collect<T: Send>(...)` generic helper |
| `std::sync::Mutex` in bounded-concurrency path | `batch.rs:254, 403` | `parking_lot::Mutex` (already a dependency) |
| `available_parallelism()` syscall per `encode_avif` | `encoder.rs:738–742` | `static AVIF_ENCODER_THREADS: OnceLock<i32>` |
| `std::env::set_var` in test `EnvGuard` is unsound in multithreaded context | `pool.rs:207–227` | Add `#[serial_test::serial]` or inject the value via a parameter |

---

## 5. Audit Report

### 5.1 `clone`

| Pattern | Locations (representative) | Count | Verdict |
|---------|---------------------------|-------|---------|
| `Arc::clone` / `source.clone()` on `Arc`-backed types | `engine.rs:70–90`, `batch.rs:69` | ~15 | **Keep** — cheap refcount increment; correct design |
| `Cow::Borrowed` clones that upgrade to `Owned` via `into_owned()` | `encoder.rs:576, 636` | ~6 | **Keep** — correct lazy ownership |
| `.clone()` on `MetadataPolicy` (recently fixed) | eliminated in b89550f | 0 | **Eliminated** — correct in current branch |
| `PreparedEncode.input_format.clone()` (`Option<String>`) | `processing.rs:329` | 1 | **Remove** — change to `&'static str` as recommended |
| EXIF `exif.to_vec()` as unconditional clone | `encoder.rs:474` | 1 | **Remove** — Cow refactor |
| ICC `icc.to_vec()` before `Bytes::from` | `encoder.rs:562, 634` | 2 | **Remove** — `Bytes::copy_from_slice` |
| `FirewallConfig.clone()` | `output_ops.rs:92,192,351,...` batch.rs | 9 | **Remove** — add `Copy` derive |

### 5.2 `Arc`

| Usage | File | Verdict |
|-------|------|---------|
| `Source::Memory/Mapped` wraps image bytes | `io/source.rs` | **Keep** — enables zero-copy task spawning |
| `PreparedEncode.processed: Arc<DynamicImage>` | `tasks/processing.rs:65` | **Keep** — shared across binary-search quality iterations |
| `OnceLock<Option<Arc<Vec<u8>>>>` for ICC/EXIF | `engine/api/engine.rs` | **Keep** — parse-once semantics |
| `Arc<std::io::Error>` in `LazyImageError` variants | `error.rs:170,175` | **Keep** — `io::Error` is not `Clone`; Arc is the correct solution |
| `Arc<AtomicUsize>` for work-steal index in `BatchTask` | `batch.rs:261` | **Keep for now** — see consolidation recommendation |

### 5.3 `Rc`

None found in production code. `Rc` is not used anywhere in `src/`.

### 5.4 `Mutex` and `RwLock`

| Location | Type | Usage | Verdict |
|----------|------|-------|---------|
| `semaphore.rs` | `parking_lot::Mutex` | Semaphore state | **Keep** — correct |
| `estimate.rs` | `parking_lot::RwLock` | Cached estimates | **Keep** — correct |
| `pool.rs:108–137` (test only) | `parking_lot::RwLock<Option<...>>` | Pool reset for tests | **Keep** — intentional double-checked lock for test isolation |
| `batch.rs:254` | `std::sync::Mutex` | Result collector, bounded path | **Replace** with `parking_lot::Mutex` |
| `batch.rs:403` | `std::sync::Mutex` | Result collector, with-metrics bounded path | **Replace** with `parking_lot::Mutex` |

### 5.5 `Box` and `dyn Trait`

| Usage | File | Verdict |
|-------|------|---------|
| `Box<dyn TaskContext>` / trait objects in NAPI task dispatch | `api/` | **Keep** — required by napi-rs `Task` trait design |
| No gratuitous `Box<dyn Error>` found | — | Clean |

### 5.6 Heap Allocations (hot-path specific)

| Allocation | Location | Verdict |
|-----------|----------|---------|
| JPEG output `Vec::with_capacity(pixels * 3 / 10)` | `encoder.rs:232` | **Keep** — correct pre-sizing |
| PNG output `Vec::new()` | `encoder.rs:532` | **Add capacity hint**: `img.width() * img.height() / 4` |
| EXIF embed `Vec::new()` | `encoder.rs:325, 342, 359` | **Add capacity hint**: `image_data.len() + exif.len() + 256` |
| ICC embed `Vec::new()` | `encoder.rs:288, 564, 636` | **Fix**: `Bytes::copy_from_slice` eliminates intermediate `Vec` |
| `fir::Resizer::new()` per resize | `resize.rs:385` | **Hoist to thread-local** |
| `sanitize_exif_bytes` `exif.to_vec()` | `encoder.rs:474` | **Remove** via `Cow` refactor |

### 5.7 `unsafe`

| File | Count | Summary | Verdict |
|------|-------|---------|---------|
| `codecs/avif_safe.rs` | 30 | RAII wrappers for libavif FFI | **Keep** — all necessary; add SAFETY comments |
| `engine/encoder.rs` | 2 | Alpha-plane write, AVIF pixel pointer | **Keep** — harden with explicit bounds check |
| `engine/io/icc.rs` | ~4 | AVIF ICC extraction, field write | **Keep** — extend SAFETY comment block |
| `engine/io/source.rs` | ~3 | mmap | **Keep** — correct with flock |
| `engine/platform.rs` | 3 | syscalls (Windows/Unix) | **Keep** — add missing SAFETY comments |

Provenance violation at `avif_safe.rs:478` (const→mut cast from shared borrow): **Fix** by calling `into_owned()` in the `Cow::Borrowed` arm.

### 5.8 `unwrap` / `expect` / `panic`

| Location | Type | Is it reachable? | Verdict |
|----------|------|-----------------|---------|
| `engine.rs:36` | `expect("failed to create napi error")` | Yes — NAPI heap failure | **Fix** — replace with safe fallback |
| `avif_safe.rs:104, 279` | `expect` in methods returning `()` | Yes — guarded by panic policy | **Fix** — return `Result` |
| `batch.rs:70` | `expect("source always has bytes")` | Future risk if new `Source` variant added | **Fix** — use `ok_or_else(...)` |
| `batch.rs:421, 427` | `unwrap()` on `Mutex` | Mutex poison on worker panic | **Fix** — match `BatchTask` pattern |
| `batch.rs:427` | `Arc::try_unwrap(out).unwrap()` | Race if rayon delays drop | **Fix** — match `BatchTask` pattern |
| `engine.rs:147, 154` | `let _ = lock.set(...)` | Unreachable today | **Fix** — use `expect` or `get_or_init` |
| `pool.rs:108–137` | `unwrap_or_else(|p| p.into_inner())` | Correctly guarded | **Keep** |
| Preset convenience methods `ops.rs:423–441` | `expect("built-in preset must exist")` | Unreachable if name constants match | **Harden** — share name constants between PRESET_TABLE and convenience methods |

---

## 6. Quality Validation

All four quality gates pass on `feature/rust-idiom-hardening`.

### 6.1 Formatting

```
Command:  cargo fmt --check
Result:   PASS
Output:   Exit code 0, no diff output
```

Every source file matches rustfmt canonical style.

### 6.2 Clippy

```
Command:  cargo clippy --no-default-features -- -D warnings
Result:   PASS
Output:   Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.86s
Warnings: 0
Errors:   0
```

### 6.3 Tests

```
Command:  cargo test --no-default-features
Result:   PASS

Binary                Test count  Result
─────────────────────────────────────────
unit (src/lib.rs)     258         ok
edge_cases            46          ok
engine_tests          64          ok
error_tests           43          ok
property_based        8           ok
─────────────────────────────────────────
Total                 419         0 failed

Doc-tests:  0 passed, 0 failed, 1 ignored
  (ignored: define_encode_task macro example requires NAPI context)
```

Three integration binaries (`fuzz_regression`, `integration_tests`, `napi_tests`) compile and run 0 tests under `--no-default-features` — expected (require native `.node` artifact or Node runtime).

### 6.4 Benchmark Compilation

```
Command:  cargo bench --no-run --no-default-features
Result:   PASS (with non-fatal deprecation warnings)

Executables produced:
  target/release/deps/benchmark-2a8fe7ee9387921c
  target/release/deps/memory_semaphore-7f24ba857869e8b2

Warnings: 9 occurrences of:
  criterion::black_box is deprecated; use std::hint::black_box() instead
  Locations: benches/benchmark.rs:6,33,54,80,81,93,94,126,180

Duration: ~92s on Apple Silicon (optimized profile, cold Cargo cache)
```

The warnings are non-blocking but should be resolved: replace `criterion::black_box` with `std::hint::black_box` at the 9 affected call sites.

---

## 7. Prioritized Refactor Roadmap

Severity levels: **Critical** (correctness/safety) → **High** (public contract or active bug) → **Medium** (quality/maintainability) → **Low** (code clarity).

Items within each phase are ordered by impact/risk ratio.

---

### Phase 1 — Safety & Correctness (no new features; address before next release)

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 1 | Fix MetadataPolicy batch GPS omission (GPS always stripped in batch regardless of policy) | `tasks/batch.rs:53–54`, `metadata.rs:14–21` | Critical | Batch respects `strip_gps: false`; integration test passes |
| 2 | Replace `napi_err().expect()` with safe fallback | `engine.rs:36` | High | No `expect` call on NAPI Result; process cannot abort from this path |
| 3 | Harden alpha-plane write loop bounds for release builds | `encoder.rs:723, 728` | High | `debug_assert!` replaced by `if ... { return Err(...) }` before the loop |
| 4 | Fix const-to-mut provenance violation in `create_rgb_image` | `avif_safe.rs:478` | High | `Cow::Borrowed` arm calls `into_owned()`; Miri or cargo-careful reports no issue |
| 5 | Make PNG quality rejection explicit | `ops.rs:261–265` | High | Passing quality to PNG returns `Err`; existing `test_png_ignores_quality` updated |
| 6 | Replace `expect("source always has bytes")` in batch worker | `batch.rs:70` | Medium | `.ok_or_else(...)` propagates through batch error collection |
| 7 | Fix Mutex bare `.unwrap()` in `BatchWithMetricsTask` | `batch.rs:421, 427` | Medium | Matches `BatchTask` poison-recovery pattern |

### Phase 2 — Error Architecture (single sitting; cascades cleanly)

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 8 | Delegate `LazyImageError::category()` to `ErrorCode::category()` | `error.rs:681–724` | High | 43-line match deleted; all existing tests pass |
| 9 | Replace manual `impl Clone` with `#[derive(Clone)]` | `error.rs:285–390` | Medium | 105 lines removed; `cargo check` clean |
| 10 | Change `FromStr::Err` from `String` to `LazyImageError` | `ops.rs:194, 235, 245` | Medium | `pipeline_ops.rs:66` `.map_err(\|_\|` removed; structured error propagated |
| 11 | Convert `avif_safe.rs` `set_color_properties` / `configure` to return `Result` | `avif_safe.rs:104, 279` | Medium | Consistent with `set_icc_profile`, `allocate_planes` peers |

### Phase 3 — Type System (can be done incrementally per type)

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 12 | `Quality` newtype | `ops.rs:300–310` | Medium | `OutputFormat` variants store `Quality`; `with_quality` constructs `Quality::unchecked` |
| 13 | `RotationAngle` enum | `ops.rs:94` | Medium | `_ => Err(...)` fallback in `apply.rs:291` removed; compiler covers all arms |
| 14 | `MetadataPolicy` as sum type | `metadata.rs:14–21` | Medium | Incoherent `gps=true, exif=false` state unrepresentable |
| 15 | `#[napi(string_enum)]` for `SanitizePolicy` and `ResizeFitMode` | `api/types.rs` | Medium | `_ =>` fallback arms in `pipeline_ops.rs:209, 64` removed |
| 16 | Rename `OutputFormat::from_str` inherent method | `ops.rs:235–237` | Low | `parse::<OutputFormat>()` no longer silently fails; naming consistent with `ResizeFit` |

### Phase 4 — Ownership & Allocation Wins

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 17 | `sanitize_exif_bytes` → `Cow<'_, [u8]>` | `encoder.rs:432–524` | Medium | No `Vec` allocation on no-op EXIF paths; criterion JPEG roundtrip benchmark validates |
| 18 | `Bytes::copy_from_slice(icc)` in PNG/WebP embed | `encoder.rs:562, 634` | Medium | Intermediate `Vec` eliminated; consistent with JPEG embed path |
| 19 | `FirewallConfig: Copy` | `firewall.rs:35–45` | Low | 9 `.clone()` calls become implicit copies; clippy passes |
| 20 | `format_to_string` → `&'static str` | `context.rs:26–39` | Low | `PreparedEncode::input_format: Option<&'static str>`; String allocated only at metrics population |
| 21 | `Vec::with_capacity` in embed paths | `encoder.rs:288, 325, 342, 359, 532` | Medium | No realloc observed under criterion throughput benchmark |
| 22 | Double `to_string_lossy()` → single capture | `batch.rs:113–116` | Low | One `to_string_lossy` call per file; diff-verifiable |

### Phase 5 — Concurrency & Performance Tuning

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 23 | `available_parallelism()` → `OnceLock<i32>` | `encoder.rs:738–742` | Medium | Syscall count drops to 1 per process; `strace`/`dtruss` confirms |
| 24 | `fir::Resizer` → thread-local | `resize.rs:385` | Medium | `fir_lanczos` criterion benchmark shows improvement for batch runs |
| 25 | `std::sync::Mutex` → `parking_lot::Mutex` in batch bounded paths | `batch.rs:254, 403` | Low | Dead poison-recovery branches removed; code consistent with rest of crate |
| 26 | Semaphore `wait_while` refactor | `semaphore.rs:121–135` | Low | `memory_semaphore` criterion benchmark shows no regression; `N-1` useless lock acquisitions eliminated |
| 27 | `std::env::set_var` → `#[serial_test::serial]` for `EnvGuard` tests | `pool.rs:207–227` | Low | No data races on `UV_THREADPOOL_SIZE` under `cargo test -- --test-threads=N` |

### Phase 6 — Unsafe Documentation & Module Structure

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 28 | Add `// SAFETY:` comments to all 30 unsafe blocks in `avif_safe.rs` | `avif_safe.rs:82–479` | Medium | `grep -n 'SAFETY' avif_safe.rs` returns ≥ 30 results |
| 29 | Elevate `create_rgb_image` to `pub unsafe fn` | `avif_safe.rs:439` | Medium | Consistent with `alpha_plane_mut`, `as_ptr`, `as_mut_ptr` |
| 30 | Add `#![deny(unsafe_op_in_unsafe_fn)]` to `src/lib.rs` | `lib.rs:1` | Low | Crate-wide lint enforced; `cargo clippy` still passes |
| 31 | Add SAFETY comments to `platform.rs` unsafe blocks | `platform.rs:76, 111, 144` | Low | `grep -n 'SAFETY' platform.rs` returns 3 results |
| 32 | Extend SAFETY comment in `icc.rs` to cover field write | `icc.rs:543` | Low | SAFETY block mentions the `maxThreads` write |
| 33 | Restore `#![cfg(feature = "napi")]` in `cgroup.rs` | `cgroup.rs:1` | Low | Defense-in-depth restored; file-level guard matches mod declaration |
| 34 | Extract EXIF/ICC embedding from `encoder.rs` to `io/exif_embed.rs` | `encoder.rs:268–524` | Medium | `encoder.rs` ≤ 600 lines; all tests pass |

### Phase 7 — Public API Polish

| Priority | Finding | File:Line | Severity | Success Criteria |
|----------|---------|-----------|----------|-----------------|
| 35 | Deprecate `preset()` in favor of `encode({ preset })` | `pipeline_ops.rs:383–421` | High | `@deprecated` JSDoc added; `toBufferWithPreset`/`encode` recommended |
| 36 | Mark `toBufferTargetBytesNative` as `@internal` | `output_ops.rs:250–256`, `index.d.ts:105` | Medium | `@internal` tag added; `format: string` → `format: OutputFormat` |
| 37 | XMP warning via `env.emit_warning()` instead of `eprintln!` | `pipeline_ops.rs:178–184` | Medium | No raw `eprintln!` in production code; warning observable via `process.on('warning')` |
| 38 | Extract `build_task_context()` helper | `output_ops.rs:62–685` | Medium | 7 duplicate `TaskContext { ... }` blocks replaced by one call; `output_ops.rs` ≤ 600 lines |
| 39 | Add real pipeline benchmarks (JPEG, WebP, resize) | `benches/benchmark.rs` | Medium | `bench_jpeg_roundtrip`, `bench_webp_encode`, `bench_resize_pipeline` groups exist and compile |
| 40 | Replace `criterion::black_box` with `std::hint::black_box` | `benches/benchmark.rs:6,33,54,80,81,93,94,126,180` | Low | 9 deprecation warnings eliminated from bench build |

---

*Review generated from adversarially-verified findings on branch `feature/rust-idiom-hardening` (HEAD: b89550f). All quality gates passed at review time.*
