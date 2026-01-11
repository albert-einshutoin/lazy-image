# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[Unreleased]: https://github.com/albert-einshutoin/lazy-image/compare/v0.8.0...HEAD
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
