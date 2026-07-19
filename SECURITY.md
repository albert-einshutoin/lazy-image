# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.14.x  | :white_check_mark: |
| 0.13.x  | :white_check_mark: |
| < 0.13  | :x:                |

## Reporting a Vulnerability

If you discover a security vulnerability, please report it privately.

**Preferred channels**
- GitHub Security Advisory: [Report a vulnerability](https://github.com/albert-einshutoin/lazy-image/security/advisories/new)
- Email: einstein.4s.1110@gmail.com (use for embargoed findings or if GSA is unavailable)

**What to include**
- Affected version(s) and environment
- Reproduction steps and minimal proof-of-concept
- Impact assessment (confidentiality/integrity/availability)
- Suggested fixes or mitigations (if any)

**Response targets**
- Acknowledge: within 72 hours
- Triage & severity: within 7 days
- Fix & patch release: depends on severity (see CVE/patch policy below)

Please avoid public issue trackers until a fix is released.

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

## CVE and Patch Policy

- **CVE assignment**: For confirmed vulnerabilities with CVSS ≥ 4.0 (Medium+), we will request a CVE via the GitHub Security Advisory workflow and publish the advisory once a fix is available.
- **Patch releases**:
  - Critical/High: security patch release within 14 days of confirmation.
  - Medium: patch in the next scheduled minor/patch release (target ≤ 30 days).
  - Low: fixed opportunistically in a regular release.
- **Backports**: Security fixes are backported to all supported lines (0.14.x and 0.13.x). Unsupported versions (<0.13) are not patched; users must upgrade.
- **Disclosure**: Public disclosure occurs after a fix is released and packages are available. If coordinated disclosure is requested, we honor reasonable embargoes up to 90 days.
- **Credit**: We credit reporters in the advisory unless anonymity is requested.

## Dependency Security Updates

Key upstreams: mozjpeg, libwebp, libavif-sys/rav1e, image crate.

### Cargo audit exceptions

CI audits both `Cargo.lock` and the independent `fuzz/Cargo.lock` through
`scripts/cargo-audit-policy.js`. Temporary advisory exceptions live in
`.github/security/cargo-audit-exceptions.json` and must include the affected
lockfile, impact assessment, upstream tracking URL, remediation plan, owner,
and a `reviewBy` date. The policy test fails once that date expires, so an
exception cannot silently become permanent. Scope an exception only to the
lockfile whose dependency path was reviewed; a new occurrence in the other
dependency graph must fail the audit.

- We monitor upstream security advisories (CVE feeds, GitHub Security Advisories, distro trackers).
- **Critical/High upstream CVEs**: update dependency and release a patched version within 14 days.
- **Medium upstream CVEs**: update in the next regular release (target ≤ 30 days).
- **Low/Informational**: update during routine maintenance.
- Dependency updates follow semver-compatible ranges where possible; otherwise, changelog will call out breaking impacts.

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
