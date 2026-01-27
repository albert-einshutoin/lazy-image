# Metadata Semantics (v0.9.x)

## Defaults
- All metadata is stripped by default for security and smaller file sizes.
- Auto-orientation is **enabled by default**: EXIF Orientation is applied, then Orientation tag is reset to 1.

## Supported Metadata Types

| Type | Support | Notes |
|------|---------|-------|
| ICC Profile | ✅ Full | Preserved with `keepMetadata({ icc: true })` |
| EXIF | ✅ Full | Preserved with `keepMetadata({ exif: true })`. Orientation auto-reset. |
| GPS | ✅ Strip by default | Stripped unless `stripGps: false` (privacy-first) |
| XMP | ⚠️ Not yet | Warning emitted, data stripped |

## Retention Options

```typescript
// Default: strip all (security-first)
ImageEngine.from(buffer).toBuffer('jpeg')

// Preserve ICC only (color accuracy)
ImageEngine.from(buffer)
  .keepMetadata({ icc: true })
  .toBuffer('jpeg')

// Preserve ICC + EXIF, strip GPS (privacy-first)
ImageEngine.from(buffer)
  .keepMetadata({ icc: true, exif: true })
  .toBuffer('jpeg')

// Preserve everything including GPS (photographers)
ImageEngine.from(buffer)
  .keepMetadata({ icc: true, exif: true, stripGps: false })
  .toBuffer('jpeg')
```

## Security Features

### GPS Stripping (Default)
GPS coordinates are stripped by default when `exif: true` is set. This protects user privacy by removing:
- GPSLatitude / GPSLongitude
- GPSAltitude
- GPSTimeStamp / GPSDateStamp

To preserve GPS data (e.g., for photography workflows), explicitly set `stripGps: false`.

### Orientation Auto-Reset
When auto-orient is enabled (default), the EXIF Orientation tag is reset to 1 after applying the rotation. This prevents double-rotation bugs that affect Sharp and other libraries.

### Firewall Integration
When `sanitize('strict')` is enabled, all metadata is stripped regardless of `keepMetadata()` settings. This ensures untrusted inputs cannot leak metadata.

## Comparison with Sharp

| Feature | Sharp | lazy-image |
|---------|-------|------------|
| Default behavior | Keeps metadata | Strips all |
| GPS stripping | Manual | Default enabled |
| Orientation reset | ❌ Bug-prone | ✅ Auto-reset |
| Firewall override | N/A | ✅ Supported |

## Format-Specific Notes

### JPEG
- ICC: Embedded in APP2 segment (ICC_PROFILE)
- EXIF: Embedded in APP1 segment

### PNG
- ICC: Embedded in iCCP chunk
- EXIF: Embedded in eXIf or zTXt chunk

### WebP
- ICC: Embedded in ICCP chunk
- EXIF: Embedded in EXIF chunk

### AVIF
- ICC: Supported via libavif
- EXIF: Not yet supported (stripped)

## mmap-related considerations
- When using `fromPath`, the source file must remain unchanged (no edits/truncate/delete) for the lifetime of the engine and any clones. On network filesystems, prefer copying to a temp path first.
