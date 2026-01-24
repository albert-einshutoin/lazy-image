# Metadata Semantics (v0.9.x)

## Defaults
- EXIF/XMP and most container metadata are stripped to minimize PII and reduce size.
- ICC color profiles are preserved for JPEG/PNG/WebP; AVIF preserves ICC when built with `libavif-sys` (v0.9.x default). Custom builds that disable libavif-sys or versions <0.9.0 will drop ICC.
- Auto-orientation is **enabled by default**: EXIF orientation is applied, then orientation metadata is removed.

## Retention options
- ICC is always preserved when present (subject to AVIF backend as noted above).
- Other metadata retention (e.g., full EXIF/XMP) is not guaranteed; current public API focuses on stripping by default for safety. Future metadata toggles will be documented separately.

## Rationale
- Stripping metadata reduces payload size, avoids leaking camera/location data, and aligns with security-first defaults.
- ICC is kept to maintain color accuracy in delivery pipelines.

## mmap-related considerations
- When using `fromPath`, the source file must remain unchanged (no edits/truncate/delete) for the lifetime of the engine and any clones. On network filesystems, prefer copying to a temp path first.

## AVIF-specific notes
- ICC preserved only when libavif backend is present (default in v0.9.x). If using ravif-only builds, convert to sRGB before encoding or upgrade.
