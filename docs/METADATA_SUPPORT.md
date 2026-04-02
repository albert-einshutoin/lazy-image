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
| EXIF | stripped | `keepMetadata({ exif: true })` | supported |
| GPS | stripped | `keepMetadata({ exif: true, stripGps: false })` | supported, privacy-first default is strip |
| XMP | stripped | `keepMetadata({ xmp: true })` | not supported, emits runtime warning |

## Output Format Matrix

| Output format | ICC | EXIF | GPS | XMP | Notes |
|---|---|---|---|---|---|
| JPEG | supported | supported | stripped by default, opt-in preserve | not supported | Orientation is normalized after auto-orient |
| PNG | ICC supported | not currently documented as preserved | n/a | not supported | PNG metadata support is narrower than JPEG |
| WebP | ICC supported | not currently documented as preserved | n/a | not supported | Favor explicit testing before relying on non-ICC metadata |
| AVIF | ICC supported on v0.9.x+ with `libavif-sys` | not currently documented as preserved | n/a | not supported | On older/ravif-only builds ICC may be dropped |

## Important Caveats

- `xmp: true` currently does **not** preserve XMP. It only emits a warning and continues processing.
- EXIF Orientation is reset to `1` after auto-orient to avoid downstream double rotation.
- `sanitize({ policy: 'strict' })` can override metadata preservation and strip metadata for safety.
- For workflows that depend on exact metadata retention, validate behavior in integration tests against your target output format.

## Recommended Usage

- Web delivery: keep the default strip behavior unless ICC/EXIF is genuinely required
- Photography / color-sensitive workflows: opt into ICC explicitly
- Privacy-sensitive uploads: keep GPS stripping enabled
- Metadata-critical pipelines: prefer JPEG if you need the clearest documented EXIF path today

## References

- [API.md](./API.md)
- [ARCHITECTURE.md](./ARCHITECTURE.md#metadata-handling)
- [MIGRATION_FROM_SHARP.md](./MIGRATION_FROM_SHARP.md)
- [README.md](../README.md)
