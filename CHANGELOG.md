# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- This CHANGELOG file to track project changes

---

## [0.7.8] - 2024-12-XX

### Changed
- CI/CD improvements: skip napi prepublish auto-publish, use manual package generation

## [0.7.7] - 2024-12-XX

### Changed
- CI/CD improvements for more reliable platform package publishing

## [0.7.6] - 2024-12-XX

### Fixed
- Fixed napi prepublish: create skeleton package.json for each platform before running prepublish

## [0.7.5] - 2024-12-XX

### Fixed
- Fixed platform-specific package publishing (robust CI/CD workflow)

## [0.7.4] - 2024-12-XX

### Fixed
- Fixed platform-specific package publishing (CI/CD improvements)

## [0.7.3] - 2024-11-XX

### Added
- Batch processing concurrency control (limit parallel workers)

### Changed
- `processBatch()` now accepts optional `concurrency` parameter

## [0.7.2] - 2024-11-XX

### Added
- Format-specific default quality settings
  - JPEG: 85 (balanced quality/size)
  - WebP: 80 (optimal for WebP compression)
  - AVIF: 60 (high efficiency, lower value still looks great)

### Changed
- Quality parameter is now optional in `toBuffer()`, `toFile()`, `toBufferWithMetrics()`

## [0.7.1] - 2024-11-XX

### Changed
- Platform-specific packages (reduced download from 42MB to ~6-9MB)
- Only download binary for current platform

## [0.7.0] - 2024-11-XX

### Added
- Built-in presets: `thumbnail`, `avatar`, `hero`, `social`
- `preset()` method returns recommended output settings

## [0.6.0] - 2024-10-XX

### Added
- Performance metrics with `toBufferWithMetrics()`
- Batch processing with `processBatch()`
- Color space API with `toColorspace()`
- Adaptive encoder settings based on quality level

## [0.5.0] - 2024-10-XX

### Added
- Memory-efficient file I/O
  - `ImageEngine.fromPath()` - read directly from filesystem
  - `toFile()` - write directly to filesystem
  - `inspectFile()` - get metadata without loading into memory

### Changed
- Recommended API for server-side processing now uses file-based methods

## [0.4.0] - 2024-09-XX

### Added
- ICC color profile preservation
- `hasIccProfile()` method to check for embedded profiles
- Automatic ICC extraction from JPEG, PNG, WebP inputs
- ICC embedding in JPEG, PNG, WebP outputs

### Notes
- AVIF format does not currently preserve ICC profiles (ravif limitation)

## [0.3.1] - 2024-09-XX

### Added
- Fast metadata inspection with `inspect()` function
- Header-only parsing for instant dimension checks

## [0.3.0] - 2024-08-XX

### Added
- AVIF format support via ravif encoder
- Next-gen image format for maximum compression

### Performance
- AVIF produces 30% smaller files than WebP
- lazy-image's AVIF is 46% smaller than sharp's AVIF

## [0.2.0] - 2024-08-XX

### Added
- Cross-platform CI/CD pipeline
- Support for macOS (Intel + Apple Silicon), Windows, Linux (glibc + musl)

## [0.1.0] - 2024-07-XX

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

[Unreleased]: https://github.com/albert-einshutoin/lazy-image/compare/v0.7.8...HEAD
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
