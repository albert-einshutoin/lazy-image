# Input Validation Rules

This project now enforces consistent runtime validation at the NAPI boundary. The rules below summarize what callers can expect; violations surface as `LazyImageError` with structured properties (`code`, `errorCode`, `category`, `recoveryHint`).

## Dimensions
- `resize(width?, height?)`: numbers must be finite integers, > 0, and ≤ `MAX_DIMENSION` (32,768). Supplying neither width nor height is rejected.
- `crop(x, y, width, height)`: offsets must be ≥ 0; width/height must be > 0 and ≤ `MAX_DIMENSION`.

## Quality and format options
- `quality` must be a finite integer ≥ 0. Values > 100 are clamped to 100 before encoding.
- `fastMode` remains a boolean; format strings are still validated against supported codecs.

## Brightness / contrast
- Values must be finite integers in the range -100..100.

## Concurrency and limits
- `processBatch` `concurrency`: finite integer, ≥ 0. `0` keeps auto-detection; values above the internal maximum (1024) are rejected.
- `limits` options (`maxPixels`, `maxBytes`, `timeoutMs`): finite integers ≥ 0. `0` disables the corresponding limit.

## Paths and file output
- `fromPath`, `inspectFile`, `toFile`, `toFileWithPreset`: paths must be non-empty. `toFile*` additionally requires the parent directory to exist; otherwise the call fails synchronously with a `UserError`.

## Error surface
- Validation errors are converted through the unified NAPI error helper, so JavaScript callers always receive `code`, `errorCode`, `category`, and `recoveryHint` fields in addition to the message.
