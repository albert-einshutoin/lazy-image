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

