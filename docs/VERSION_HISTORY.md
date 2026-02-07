# Version History

| Version | Features |
|---------|----------|
| v0.9.0 | Streaming architecture, EXIF auto-orientation, Image Firewall, Quality metrics (SSIM/PSNR), Metrics API v1.0.0, Memory estimate model, Weighted semaphore, Golden test suite, Parallel resize, PNG compression with oxipng, Unified format detection, Cgroup detection, Fuzzing CI |
| v0.8.7 | Telemetry metrics, Smart concurrency with auto memory cap detection, Performance optimizations |
| v0.8.6 | Version bump to 0.8.6 |
| v0.8.5 | Fixed CI compilation errors and improved --no-default-features build support |
| v0.8.4 | Zero-copy memory mapping implementation: fromPath() and processBatch() use mmap for zero-copy file access |
| v0.8.3 | Documentation: Updated README.md to document zero-copy memory mapping for processBatch() |
| v0.8.1 | WebP encoding optimization: ~4x speed improvement (method 4, single pass) to match sharp performance |
| v0.8.0 | Updated benchmark results, improved test suite |
| v0.7.7 | CI/CD improvements: skip napi prepublish auto-publish, use manual package generation |
| v0.7.6 | Fixed napi prepublish: create skeleton package.json for each platform before running prepublish |
| v0.7.5 | Fixed platform-specific package publishing (robust CI/CD workflow) |
| v0.7.4 | Fixed platform-specific package publishing (CI/CD improvements) |
| v0.7.3 | Batch processing concurrency control (limit parallel workers) |
| v0.7.2 | Format-specific default quality (JPEG: 85, WebP: 80, AVIF: 60) |
| v0.7.1 | Platform-specific packages (reduced download from 42MB to ~6-9MB) |
| v0.7.0 | Built-in presets (`thumbnail`, `avatar`, `hero`, `social`) |
| v0.6.0 | Performance metrics (`toBufferWithMetrics`), batch processing (`processBatch`), color space API, adaptive encoder settings |
| v0.5.0 | Memory-efficient file I/O (`fromPath`, `toFile`, `inspectFile`) |
| v0.4.0 | ICC color profile preservation |
| v0.3.1 | Fast metadata (`inspect`) |
| v0.3.0 | AVIF support |
| v0.2.0 | Cross-platform CI/CD |
| v0.1.0 | Initial release |
