# Error Semantics (v0.9.x)

lazy-image exposes structured errors with category + code for programmatic handling.

## Taxonomy
- **UserError**: caller-provided input is invalid; recoverable by fixing parameters or files.
- **CodecError**: decode/encode/format issues (corrupt data, unsupported format, codec failure).
- **ResourceLimit**: dimension/pixel/byte/time/concurrency limits, or I/O failures tied to resource pressure.
- **InternalBug**: unexpected internal state or dependency panic; should be reported.

## Error codes
- Ranges: `E1xx` Input, `E2xx` Processing, `E3xx` Output, `E4xx` Configuration, `E9xx` Internal.
- JS errors carry `error.code` (e.g., `LAZY_IMAGE_USER_ERROR`) and `error.category`; `getErrorCategory()` maps them to enums.

## Common mappings
- Dimension > limit → `ResourceLimit / DimensionExceedsLimit` (E121).
- Pixels > limit → `ResourceLimit / PixelCountExceedsLimit` (E122).
- Invalid resize/rotation/crop → `UserError / Invalid*` (E200/E201).
- Unsupported/invalid format → `CodecError / UnsupportedFormat` (E111) or `DecodeFailed/EncodeFailed` (E300/E301 range).
- Firewall violations (bytes/pixels/metadata/timeout) → `ResourceLimit / FirewallViolation`.
- Dependency panic (mozjpeg/libavif/etc.) → `InternalBug / InternalPanic` (E900s) via panic guard.

## Batch/streaming
- `processBatch` returns per-file `error`, `errorCode`, `errorCategory` in `BatchResult` entries; success/failure is per item.
- Streaming pipeline surfaces errors via stream `error` events and destroys the output stream on failure.

## Clone semantics
- `clone()` duplicates the engine state by sharing immutable data via `Arc`:
  - `source` (mmap/buffer) is shared; modifications to the underlying file after cloning are unsafe and may affect all clones.
  - `decoded` image is shared until a mutation requires a copy (Copy-on-Write via `Arc` + `Cow`).
  - `ops` queue is cloned per engine; subsequent `resize/crop/...` calls are isolated per clone.
  - `icc_profile` is shared as immutable bytes.
- Safe pattern: decode once, clone, then diverge ops to emit multiple outputs without re-decoding.
- Unsafe pattern: modify/delete mmap'ed file after cloning; may SIGBUS or corrupt all clones.
