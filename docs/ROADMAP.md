# lazy-image Roadmap

lazy-image is a **web image optimization engine**, not a general-purpose image editor.  
This roadmap defines **what we build**, **what we reject**, and **how decisions are made**.

---

## 1. Project Direction (What lazy-image IS)

lazy-image focuses on:

- **Web-first formats:** JPEG / WebP / AVIF  
- **Basic geometric operations:** resize, crop, rotate, flip  
- **Color correctness:** ICC handling, safe defaults  
- **Memory efficiency:** file-based I/O, minimal V8 heap usage  
- **Small, stable API:** `from → pipeline → toBuffer/toFile`

The goal is simple:

> **Produce the smallest, highest-quality web images safely and fast,  
> with a minimal and predictable API.**

---

## 2. Non-Goals (What lazy-image will NOT become)

To avoid feature bloat, lazy-image will **not** add:

- ❌ Text rendering, drawing, canvas primitives  
- ❌ Heavy editing filters (blur, sharpen, artistic effects)  
- ❌ GIF/APNG animation or video processing  
- ❌ Full feature parity with sharp/jimp  
- ❌ Real-time <10ms image mutation at high concurrency  

If a feature moves lazy-image toward “Photoshop-in-Node”,  
it will not be accepted.

---

## 3. Feature Acceptance Rules (Decision Framework)

New features must meet **all** of these conditions:

1. **Directly improves web image optimization**  
   (file size, quality, memory, pipeline performance)

2. **Fits the minimal API philosophy**  
   (no complex abstractions, no feature creep)

3. **Does not compromise speed or memory usage**

4. **Has a real-world use case in web delivery**  
   (CDN, upload pipeline, build-time optimization)

5. **Does not push lazy-image toward sharp/jimp territory**  
   (canvas, filters, rich graphics = reject)

If the answer to any of these is “no”, the feature is rejected.

---

## 4. High-Level Roadmap

### 0.7.x
- Built-in presets (thumbnail / avatar / hero / social-card)
- More consistent defaults for JPEG/WebP/AVIF
- Improved batch processing (concurrency tuning, better output)

### 0.8.x (Production Readiness)
**Goal: Fix architectural issues and improve production reliability**

#### Error Handling Overhaul ✅ **Completed**
- Replace string-based errors with structured error types
  - `DecodeError`, `EncodeError`, `InvalidICCProfile`, `DimensionTooLarge`, `UnimplementedFormat`
  - Error codes and proper error classification
  - Better error messages with context
- **Status**: Implemented in v0.7.x - Error code system (E1xx-E9xx) with categorized errors
- **Why**: Current `Error::from_reason()` approach loses type safety and makes error handling difficult

#### Documentation & Transparency ✅ **Completed**
- Explicitly document input format limitations
  - 16bit images are converted to 8bit (by design, not a bug)
  - Clear limitations section in README
- **Status**: README now includes Limitations section, ERROR_CODES.md documented
- **Why**: Users need to know limitations upfront, not discover them at runtime

#### Thread Model Safety ✅ **Completed**
- Fix dual thread pool issue (libuv + rayon)
  - Design clear thread usage strategy
  - Default concurrency = CPU cores
  - For batch processing: use rayon threads exclusively, minimize libuv usage
  - Document thread model and Docker/CPU-limited environment behavior
- **Status**: Documented in `docs/THREAD_MODEL.md`, concurrency control added in v0.7.3
- **Why**: Current model can cause unpredictable scheduling and thread saturation under load

#### Encoder Parameter Control ❌ **Not Started**
- Improve quality-to-encoder-parameters mapping
  - JPEG: SSIM-based quality adjustment
  - WebP: image complexity-aware method optimization
  - AVIF: content-adaptive encoding
- Move beyond "fixed threshold" approach
- **Status**: Not started - requires significant research and implementation effort
- **Why**: Single quality parameter is insufficient for optimal compression across formats

#### API Consistency & Maintainability ✅ **Completed**
- Improve internal Rust API consistency
  - Standardize error handling patterns
  - Improve code organization for maintainability
  - Better separation of concerns
- **Status**: Error handling standardized, code organization improved
- **Why**: Current implementation works but is hard to maintain and debug

#### Memory Efficiency for Large Images ❌ **Not Started**
- Address memory pressure in high-concurrency scenarios
  - 50MP × 10 parallel processing on 4-8GB RAM servers
  - Improve memory usage patterns
  - Consider streaming/chunked processing for very large images (if justified)
- **Status**: Not started - current implementation works for typical web image sizes
- **Why**: Current "full decode → full hold → process → re-encode" model can fail under memory pressure

#### API Design Decisions ✅ **Completed**
- **A-001: toBuffer() Non-destructive Behavior Review** ✅ **Completed**
  - Created ADR-001 and decided on non-destructive behavior
  - Compared Option 1 (maintain status quo), Option 2 (non-destructive), Option 3 (provide both)
  - Adopted Option 2 (non-destructive): Changed `take()` to `clone()`
  - Memory efficiency impact is limited (only reference count increases via `Arc`)
  - See `docs/ADR-001-toBuffer-destructive-behavior.md` for details
- **Why**: Need to finalize important API design decisions before v1.0

### 1.0
- API surface freeze (no breaking changes)
- Stability across platforms (macOS/Win/Linux)
- Deployment guides (Docker, Lambda)

### Post-1.0 (Optional, only if justified by rules above)
- Smarter encoder strategies  
- Higher-level helpers for responsive images

---

## 5. Contribution Notes

Before proposing a feature:

- Describe the **use case**, not the API.
- Confirm it matches the **Feature Acceptance Rules**.
- If unsure, open an issue titled **“Proposal: <feature> (use case only)”**.

lazy-image succeeds by staying focused.  
Everything else belongs in a separate library.

