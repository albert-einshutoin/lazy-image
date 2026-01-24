# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.9.x   | :white_check_mark: |
| 0.8.x   | :white_check_mark: |
| < 0.8   | :x:                |

## Security Measures

lazy-image implements several security measures to protect against common vulnerabilities:

### Decompression Bomb Protection

- **Maximum dimension limit**: 32,768 × 32,768 pixels (same as libvips/sharp)
- **Maximum pixel count**: 100 megapixels (≈400MB uncompressed RGBA)
- Images exceeding these limits are rejected before processing

### Memory Safety

- Core processing is written in Rust, providing memory safety guarantees
- Panic handling prevents crashes from propagating to Node.js
- File-based I/O (`fromPath`/`toFile`) bypasses V8 heap, reducing memory pressure
- Automated leak detection: CI runs AddressSanitizer and Valgrind against the
  Rust-only stress harness (`examples/stress_test.rs`) to catch regressions

### Input Validation

- Image format detection by magic bytes, not file extension
- Structured error handling with error codes for programmatic handling
- Graceful handling of corrupted or malformed images

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly:

### DO

1. **Email directly**: Report security issues privately to the maintainers
2. **Include details**: Provide steps to reproduce, impact assessment, and any proof-of-concept
3. **Allow time**: Give us reasonable time to address the issue before public disclosure

### DON'T

- Open a public GitHub issue for security vulnerabilities
- Disclose the vulnerability publicly before it's fixed
- Exploit the vulnerability beyond what's needed to demonstrate it

### Response Timeline

- **Initial response**: Within 72 hours
- **Triage and assessment**: Within 1 week
- **Fix development**: Depends on severity
  - Critical/High: Target fix within 1-2 weeks
  - Medium: Target fix in next minor release
  - Low: Target fix in next major release

### After Reporting

1. We will acknowledge receipt of your report
2. We will investigate and validate the vulnerability
3. We will develop and test a fix
4. We will release a patched version
5. We will credit you in the release notes (unless you prefer to remain anonymous)

## Security Best Practices for Users

### Input Validation

Always validate user-uploaded images before processing:

```javascript
const { inspectFile } = require('@alberteinshutoin/lazy-image');

// Check metadata before processing
const meta = inspectFile(userUploadPath);

// Reject oversized images
const MAX_DIMENSION = 10000;
if (meta.width > MAX_DIMENSION || meta.height > MAX_DIMENSION) {
  throw new Error('Image too large');
}

// Reject unsupported formats
const ALLOWED_FORMATS = ['jpeg', 'png', 'webp'];
if (!ALLOWED_FORMATS.includes(meta.format)) {
  throw new Error('Unsupported format');
}
```

### Resource Limits

When processing untrusted images, consider:

```javascript
// Use concurrency limits for batch processing
const results = await engine.processBatch(
  files,
  outputDir,
  'webp',
  80,
  4  // Limit concurrent processing
);
```

### File System Security

- Validate output paths to prevent path traversal
- Use temporary directories for intermediate files
- Set appropriate file permissions on output files

## Dependencies

lazy-image uses the following image processing libraries:

| Library | Purpose | Security |
|---------|---------|----------|
| mozjpeg | JPEG encoding | Actively maintained, fuzzing tested |
| libwebp | WebP encoding | Google-maintained, part of Chrome |
| ravif | AVIF encoding | Pure Rust, memory-safe |
| image | Image decoding | Rust ecosystem, security-focused |

We monitor security advisories for all dependencies and update promptly when vulnerabilities are discovered.

## Contact

For security-related inquiries:
- **Email**: einstein.4s.1110@gmail.com (preferred for sensitive reports)
- **GitHub Security Advisories**: [Report a vulnerability](https://github.com/albert-einshutoin/lazy-image/security/advisories/new)

Thank you for helping keep lazy-image secure! 🔒
