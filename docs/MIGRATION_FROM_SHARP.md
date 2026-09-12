# Migrating a sharp workflow

Keep the operations your application needs and port a bounded workflow first.
lazy-image is not a drop-in replacement. If the goal is a verified public image
set, start with [compileImage](./ADOPTION_GUIDE.md) instead of translating each
output call. Keep rich editing with your existing tool when needed.

## Map the common operations

| sharp | lazy-image | Difference to account for |
|---|---|---|
| `sharp(buffer)` | `ImageEngine.from(buffer)` | Copies the supplied bytes into Rust-owned memory |
| `sharp(path)` | `await ImageEngine.fromPathAsync(path)` | Recommended server constructor; source setup completes asynchronously |
| `.resize({ width: 800, fit: 'inside' })` | `.resize({ width: 800, fit: 'inside' })` | Specify fit explicitly when porting |
| `.extract({ left: 10, top: 20, width: 300, height: 200 })` | `.crop(10, 20, 300, 200)` | Coordinates start at top-left; check bounds after prior operations |
| `.rotate(90)` | `.rotate(90)` | lazy-image supports only 90/180/270 degree rotation |
| `.flip().flop()` | `.flipV().flipH()` | Vertical then horizontal flip |
| `.webp({ quality: 80 }).toFile(path)` | `.toFile(path, 'webp', 80)` | Format is explicit; returns bytes written rather than sharp's info object |
| `.toBuffer({ resolveWithObject: true })` | `.toBufferWithMetrics('webp', 80)` | Returns `{ data, metrics }`, not `{ data, info }` |
| `.clone()` | `.clone()` | Branch the configured pipeline for multiple outputs |

Do not substitute contrast for saturation or assume brightness controls have
identical formulas. Unsupported editing operations need an explicit design choice,
not an approximate method mapping.

## Check behavior, not just method names

- **Resize:** lazy-image defaults to `inside`; sharp defaults to `cover` when both
  dimensions are supplied. Use explicit fit values. See [sharp resize](https://sharp.pixelplumbing.com/api-resize/).
- **Orientation:** lazy-image auto-orients during decode. In sharp, use explicit
  `autoOrient()` or the appropriate constructor option when that behavior is needed.
  See [sharp operations](https://sharp.pixelplumbing.com/api-operation/#autoorient).
- **Metadata:** both strip metadata by default. lazy-image's `keepMetadata()` keeps
  GPS stripping unless opted out; `public-upload` forces its privacy policy.
  See [metadata contract](./METADATA_SUPPORT.md) for ICC/EXIF restrictions.
- **Quality:** use explicit values as starting points, then inspect your outputs.
  The same number is not perceptual parity across encoders. See [quality](./QUALITY_SEMANTICS.md).
- **Memory:** input Buffer bytes are copied into Rust, not into V8. File APIs avoid
  that Buffer path but still need native memory. See [file memory](./ZERO_COPY.md).
- **Streaming:** the lazy-image helper stages input and output on disk; do not
  assume that stream-shaped I/O means incremental image processing.

## Example

These use explicit geometry and encoding options; they do not promise identical
pixels, file sizes or execution time.

```javascript
// Existing sharp pipeline
import sharp from 'sharp';
await sharp('input.jpg').autoOrient()
  .resize({ width: 800, height: 600, fit: 'inside' })
  .webp({ quality: 80 }).toFile('sharp-output.webp');
```

```javascript
// lazy-image pipeline (.mjs, top-level await)
import { ImageEngine } from '@alberteinshutoin/lazy-image';
const image = await ImageEngine.fromPathAsync('input.jpg');
await image.resize({ width: 800, height: 600, fit: 'inside' })
  .toFile('lazy-output.webp', 'webp', 80);
```

Validate dimensions, orientation, transparency, metadata, errors and resource
costs on your inputs before switching. [Performance](./PERFORMANCE.md) explains
the historical evidence and its limits; [compatibility](./COMPATIBILITY.md) lists
unsupported formats and operations. Sharp references checked 2026-09-13.
