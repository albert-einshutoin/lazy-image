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

For untrusted public uploads, use `sanitize({ policy: 'public-upload' })`.
It keeps the strict 32 MiB / 40 MP / cooperative 5-second resource limits,
preserves only structurally valid ICC profiles up to 512 KiB, and forces all
EXIF (including GPS), XMP, comments, and unknown ancillary metadata out of the
re-encoded output. Calling `keepMetadata()` before or after cannot weaken this
policy; `.limits()` may only tighten its fixed resource limits.

`compileImage()` applies the same privacy-safe policy to every delivery
artifact, records the existing `IccOutcome` value in the manifest, and uses a
separate strip-all metadata policy for the 16px placeholder. The placeholder
is the only artifact read into a bounded Node.js buffer (to build its data URL).

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
- `sanitize({ policy: 'public-upload' })` forces ICC-only output metadata. Malformed ICC is reported as `unsafe-stripped`; oversized ICC is rejected by the firewall.
- For workflows that depend on exact metadata retention, validate behavior in integration tests against your target output format.

## Inspection is separate from preservation

`inspect()` and `inspectFile()` expose EXIF Orientation as a preflight fact;
they do not preserve metadata or transform dimensions. The returned
`orientation` is `1` through `8` when a valid primary-image tag is present and
found within the 64 KiB preflight scan budget, and `undefined` otherwise. The
budget prevents an EXIF-free container from forcing inspection to consume the
encoded image payload. `width` and `height` remain the encoded dimensions.

The same successful preflight also returns authoritative `hasAlpha` and
`isAnimated` booleans for JPEG, PNG, and WebP. A malformed or unsupported
container fails through the typed error contract instead of turning an unknown
trait into `false`. Output metadata stripping, GPS policy, ICC preservation,
and auto-orient behavior remain controlled by the processing APIs described
above.

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
