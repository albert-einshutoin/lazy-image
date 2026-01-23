# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Performance
- Optimized codec backends (PNG decode via zune-png, WebP decode via libwebp)
  - Faster PNG/WebP decoding with SIMD/native codecs
  - Fallback to image crate for large PNGs (>16,384px) and animated WebP to preserve compatibility
  - Added safety checks to keep MAX_DIMENSION enforcement consistent

### Added
- Fused Extract operation (resize+crop) with zero-allocation pipeline path and memory model support (#240)
- Benchmarks for resize+crop vs sharp plus JS integration tests covering fusion path (#240)

### Changed
- Memory semaphore switched to `parking_lot` mutex/condvar for reduced contention under load; added contention benchmark (#241)
- Added ColorState tracking for pipeline operations (color space / bit depth / transfer / ICC) to prepare for safer color-handling (#169)

---

## [0.9.0] - 2026-01-21

### Added
- Streaming architecture for bounded-memory processing (#228, #239)
  - Added `createStreamingPipeline()` function for disk-backed streaming processing
  - Supports processing huge images without loading into memory
  - Ideal for serverless and memory-constrained environments
- EXIF auto-orientation enabled by default (#231)
  - Images are automatically rotated based on EXIF orientation tag
  - Use `autoOrient(false)` to opt-out
- Image Firewall for production-ready input sanitization (#213, #175)
  - Strict and lenient policies for protecting against decompression bombs
  - Configurable limits for pixels, bytes, and processing time
  - Automatic rejection of malicious images
- Quality metrics (SSIM/PSNR) for image quality assessment (#173, #235)
  - Added quality metrics calculation for benchmarking
  - SSIM ≥ 0.995 and PSNR ≥ 40 dB quality gates
- Metrics API v1.0.0 for production monitoring (#194, #234, #225, #182)
  - Unified metrics schema with versioning
  - Comprehensive performance tracking (decode, process, encode times)
  - Memory usage and compression ratio tracking
- Memory estimate model for intelligent resource management (#174, #233)
  - Predicts memory usage before processing
  - Helps prevent OOM in constrained environments
- Weighted semaphore for memory backpressure (#227, #232)
  - Prevents memory exhaustion during batch processing
  - Automatic concurrency adjustment based on available memory
- Golden test suite for regression testing (#230, #236)
  - Ensures consistent output quality across versions
  - Validates file size and visual quality
- Parallel resize with Rayon for improved performance (#222)
  - SIMD-accelerated parallel resizing
  - Better utilization of multi-core CPUs
- PNG compression with oxipng (#224, #148)
  - Improved PNG file size optimization
  - Better compression ratios for PNG outputs
- Quality/effort/speed mapping for optimal encoder settings (#223, #185)
  - Intelligent quality parameter mapping
  - Format-specific optimization strategies
- Unified format detection (#238)
  - Consistent format detection across all entry points
  - Improved reliability and performance
- Cgroup detection for container memory limits (#191, #237)
  - Automatic detection of container memory constraints
  - Better concurrency management in Docker/Kubernetes
- Fuzzing CI integration (#214, #179)
  - Automated fuzzing tests in CI pipeline
  - Improved security and stability
- Property-based testing with proptest (#219, #177)
  - Comprehensive test coverage
  - Better edge case detection

### Changed
- Project positioning and documentation updates (#221, #160)
  - Clarified positioning vs sharp
  - Updated compatibility matrix
- Improved error handling and error taxonomy (#171)
  - Structured error codes for better error handling
  - Clear error categories (UserError, CodecError, ResourceLimit, InternalBug)
- Specified previously unspecified behavior (#220, #178)
  - Documented edge cases and undefined behavior
  - Improved API consistency

### Fixed
- EXIFパーサ依存をcrates.io配布の`kamadak-exif`に変更しビルド再現性を確保 (#231)
- Fixed PNG ICC profile tests (#218, #164)
- Fixed tests that were always passing (#217, #216)

---

## [0.8.7] - 2026-01-13

### Added
- Telemetry metrics for performance monitoring (#86)
  - Added `toBufferWithMetrics()` method to return processing metrics
  - Added `ProcessingMetrics` interface with decode time, process time, encode time, memory peak, CPU time, and compression ratio
  - Added `OutputWithMetrics` interface combining output data and metrics
- Smart concurrency with auto memory cap detection (#85)
  - Automatic detection of container memory limits (cgroup v1/v2 support)
  - Memory-aware concurrency adjustment for `processBatch()` to prevent OOM kills
  - Automatic thread pool sizing based on available memory in constrained environments

### Changed
- Performance optimizations (#156)
  - Optimized AVIF encoding speed preset
  - Added JPEG fast mode support
  - Improved jemalloc configuration with `disable_initial_exec_tls` flag

---

## [0.8.6] - 2026-01-XX

### Changed
- Version bump to 0.8.6

---

## [0.8.5] - 2026-01-12

### Fixed
- Fixed CI compilation errors when building with `--no-default-features`
- Made imports conditional for NAPI-dependent types to support compilation without default features
- Fixed missing export of `run_stress_iteration` from engine module for stress testing

### Changed
- Updated conditional compilation attributes to properly handle `--no-default-features` builds
- Improved test module imports in `engine.rs` for better feature flag compatibility

---

## [0.8.4] - 2026-01-12

### Added
- Zero-copy memory mapping implementation (#128)
  - `fromPath()` now uses memory mapping (mmap) for zero-copy file access
  - `processBatch()` uses memory mapping for efficient batch processing
  - Bypasses Node.js heap entirely, ideal for processing large images in memory-constrained environments
  - Added `MmapFailed` error type for accurate error reporting

### Changed
- Improved zero-copy implementation: eliminated unnecessary buffer copies for memory-mapped sources
- Updated `BatchTask` to use memory mapping instead of `fs::read` for zero-copy access
- Enhanced documentation in README.md with detailed zero-copy memory mapping information
- Added Windows file locking notes in README.md (memory-mapped files cannot be deleted while mapped on Windows)

### Fixed
- Fixed issue where `Source::load()` was converting Mapped→Vec, defeating zero-copy purpose
- Removed dead code: `Source::Path` variant and `source_bytes` field

---

## [0.8.3] - 2026-01-12

### Documentation
- Updated README.md to document zero-copy memory mapping for `processBatch()`
  - Added clarification that `processBatch()` uses memory mapping (mmap) for zero-copy file access
  - Updated Memory Management section to explicitly mention both `fromPath()` and `processBatch()` use zero-copy memory mapping
  - This makes it clear that batch processing bypasses the Node.js heap entirely, ideal for memory-constrained environments

---

## [0.8.2] - 2026-01-08

### Changed
- Updated benchmark results in README.md with actual Docker environment data
  - AVIF: 6.3x faster than sharp (was 1.70x) for large files
  - JPEG: 20-25% smaller files (was 46%)
  - Added WebP benchmark results
  - Clarified resize method: 800×600 (fit inside) instead of 800px width
- Added reference to [lazy-image-test](https://github.com/albert-einshutoin/lazy-image-test) Docker repository for reproducible benchmarks

### Added
- Type-safe API definitions for API freeze preparation (#84)
  - `InputFormat`, `OutputFormat`, `PresetName` type definitions
  - Case-insensitive input support for better developer experience
- Cross-platform CI testing: Rust tests now run on both Ubuntu and macOS

### Fixed
- Fixed alignment error handling for fast_image_resize v5 (#109)
- Removed unnecessary `num_cpus` dependency, using standard library instead (#110)

### Improved
- CI: Rust tests now use matrix strategy (ubuntu-latest + macos-14) for better cross-platform coverage

---

## [0.8.1] - 2026-01-01

### Performance
- Optimized WebP encoding settings to match sharp performance
  - Changed method from 5-6 to 4 (balanced, sharp-equivalent)
  - Changed pass from 3-5 to 1 (single pass, ~3-5x faster)
  - Changed preprocessing from 1-3 to 0 (disabled, ~10-15% faster)
  - Expected ~4x speed improvement while maintaining quality parity with sharp

### Fixed
- Resolves issue #74: WebP encoding speed optimization

---

## [0.8.0] - 2025-12-28

### Changed
- Updated benchmark results with latest comparison against sharp v0.34.x
- README.md benchmark section now reflects current performance characteristics
- Improved benchmark test image (66MB PNG with complex patterns)

### Deprecated
- `toColorspace()` method: Strengthened deprecation warning. **Will be removed in v1.0**. This method only ensures RGB/RGBA format, not true color space conversion.

---

## [0.7.9] - 2025-12-09

### Changed
- Documentation and project improvements

## [0.7.8] - 2025-12-01

### Changed
- CI/CD improvements: skip napi prepublish auto-publish, use manual package generation

## [0.7.7] - 2025-12-01

### Changed
- CI/CD improvements for more reliable platform package publishing

## [0.7.6] - 2025-11-30

### Fixed
- Fixed napi prepublish: create skeleton package.json for each platform before running prepublish

## [0.7.5] - 2025-11-30

### Fixed
- Fixed platform-specific package publishing (robust CI/CD workflow)

## [0.7.4] - 2025-11-30

### Fixed
- Fixed platform-specific package publishing (CI/CD improvements)

## [0.7.3] - 2025-11-30

### Added
- Batch processing concurrency control (limit parallel workers)

### Changed
- `processBatch()` now accepts optional `concurrency` parameter

## [0.7.2] - 2025-11-30

### Added
- Format-specific default quality settings
  - JPEG: 85 (balanced quality/size)
  - WebP: 80 (optimal for WebP compression)
  - AVIF: 60 (high efficiency, lower value still looks great)

### Changed
- Quality parameter is now optional in `toBuffer()`, `toFile()`, `toBufferWithMetrics()`

## [0.7.1] - 2025-11-30

### Changed
- Platform-specific packages (reduced download from 42MB to ~6-9MB)
- Only download binary for current platform

## [0.7.0] - 2025-11-30

### Added
- Built-in presets: `thumbnail`, `avatar`, `hero`, `social`
- `preset()` method returns recommended output settings

## [0.6.0] - 2025-11-30

### Added
- Performance metrics with `toBufferWithMetrics()`
- Batch processing with `processBatch()`
- Color space API with `toColorspace()`
- Adaptive encoder settings based on quality level

## [0.5.0] - 2025-11-28

### Added
- Memory-efficient file I/O
  - `ImageEngine.fromPath()` - read directly from filesystem
  - `toFile()` - write directly to filesystem
  - `inspectFile()` - get metadata without loading into memory

### Changed
- Recommended API for server-side processing now uses file-based methods

## [0.4.0] - 2025-11-28

### Added
- ICC color profile preservation
- `hasIccProfile()` method to check for embedded profiles
- Automatic ICC extraction from JPEG, PNG, WebP inputs
- ICC embedding in JPEG, PNG, WebP outputs

### Notes
- AVIF format does not currently preserve ICC profiles (ravif limitation)

## [0.3.1] - 2025-11-28

### Added
- Fast metadata inspection with `inspect()` function
- Header-only parsing for instant dimension checks

## [0.3.0] - 2025-11-28

### Added
- AVIF format support via ravif encoder
- Next-gen image format for maximum compression

### Performance
- AVIF produces 30% smaller files than WebP
- lazy-image's AVIF is 46% smaller than sharp's AVIF

## [0.2.0] - 2025-11-28

### Added
- Cross-platform CI/CD pipeline
- Support for macOS (Intel + Apple Silicon), Windows, Linux (glibc + musl)

## [0.1.0] - 2025-11-28

### Added
- Initial release
- Core image processing engine with Rust + NAPI-RS
- JPEG encoding with mozjpeg (progressive, optimized Huffman tables)
- WebP encoding with libwebp
- PNG encoding
- Pipeline operations: resize, crop, rotate, flip, grayscale, brightness, contrast
- Fluent API with method chaining
- Lazy pipeline execution
- Async/Promise-based API

---

[Unreleased]: https://github.com/albert-einshutoin/lazy-image/compare/v0.9.0...HEAD
[0.9.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.7...v0.9.0
[0.8.7]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.6...v0.8.7
[0.8.6]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.5...v0.8.6
[0.8.5]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.4...v0.8.5
[0.8.4]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.9...v0.8.0
[0.7.9]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.8...v0.7.9
[0.7.8]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.7...v0.7.8
[0.7.7]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.6...v0.7.7
[0.7.6]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.5...v0.7.6
[0.7.5]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.4...v0.7.5
[0.7.4]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.3...v0.7.4
[0.7.3]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/albert-einshutoin/lazy-image/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/albert-einshutoin/lazy-image/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/albert-einshutoin/lazy-image/releases/tag/v0.1.0
