# Metadata Support Matrix

This document is the canonical reference for metadata behavior in lazy-image.

## Default Behavior

By default, lazy-image strips metadata for safety and smaller outputs.

- ICC: stripped unless explicitly preserved
- EXIF: stripped unless explicitly preserved
- GPS: stripped by default even when EXIF is preserved
- XMP: not currently preserved

Use `keepMetadata()` to opt into preservation where supported.

```javascript
await ImageEngine.fromPath('input.jpg')
  .keepMetadata({ icc: true, exif: true })
  .toFile('output.jpg', 'jpeg', 85);
```

## Feature Matrix

| Metadata type | Default | Opt-in API | Current status |
|---|---|---|---|
| ICC | stripped | `keepMetadata({ icc: true })` | supported |
| EXIF | stripped | `keepMetadata({ exif: true })` | supported for extractable JPEG EXIF |
| GPS | stripped | `keepMetadata({ exif: true, stripGps: false })` | supported, privacy-first default is strip |
| XMP | stripped | `keepMetadata({ xmp: true })` | not supported, emits runtime warning |

## Output Format Matrix

| Output format | ICC | EXIF | GPS | XMP | Notes |
|---|---|---|---|---|---|
| JPEG | supported | supported | stripped by default, opt-in preserve | not supported | Orientation is normalized after auto-orient |
| PNG | supported | supported via `eXIf` chunk | stripped by default when EXIF is preserved | not supported | Orientation is normalized after auto-orient |
| WebP | supported | supported via `EXIF` chunk | stripped by default when EXIF is preserved | not supported | Orientation is normalized after auto-orient |
| AVIF | supported on v0.9.x+ with `libavif-sys` | not supported | n/a | not supported | On older/alternate backend builds ICC may be dropped |

## Important Caveats

- `xmp: true` currently does **not** preserve XMP. It only emits a warning and continues processing.
- JPEG, PNG, and WebP EXIF preservation is a documented support guarantee when `keepMetadata({ exif: true })` is set and the input is a JPEG with extractable EXIF. EXIF extraction is capped at 8 MiB of source data to avoid copying unbounded metadata from oversized inputs.
- lazy-image writes sanitized EXIF into JPEG APP1, PNG `eXIf`, and WebP `EXIF` containers.
- EXIF Orientation is reset to `1` after auto-orient for JPEG, PNG, and WebP outputs to avoid downstream double rotation.
- GPS data inside EXIF is stripped by default for JPEG, PNG, and WebP outputs. Use `keepMetadata({ exif: true, stripGps: false })` only when location metadata must be retained.
- `sanitize({ policy: 'strict' })` can override metadata preservation and strip metadata for safety.
- For workflows that depend on exact metadata retention, validate behavior in integration tests against your target output format.

## Recommended Usage

- Web delivery: keep the default strip behavior unless ICC/EXIF is genuinely required
- Photography / color-sensitive workflows: opt into ICC explicitly
- Privacy-sensitive uploads: keep GPS stripping enabled
- Metadata-critical pipelines: JPEG, PNG, and WebP have documented EXIF preservation paths; prefer JPEG if downstream tools do not read PNG `eXIf` or WebP `EXIF` chunks reliably

## References

- [API.md](./API.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md#metadata-handling)
- [MIGRATION_FROM_SHARP.md](./MIGRATION_FROM_SHARP.md)
- [README.md](../README.md)
