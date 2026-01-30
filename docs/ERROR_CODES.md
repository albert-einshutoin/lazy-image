# Error Codes Reference

lazy-image uses a structured error code system for type-safe error handling. All errors are categorized and include detailed context information.

## Error Taxonomy

lazy-image uses a 4-tier error taxonomy to enable proper error handling in JavaScript:

| Category | Description | Recoverable | Example |
|----------|-------------|-------------|---------|
| **UserError** | Invalid input, recoverable by user | Yes | Invalid rotation angle, file not found, invalid crop bounds |
| **CodecError** | Format/encoding issues | Usually No | Unsupported format, corrupted image, encode/decode failures |
| **ResourceLimit** | Memory/time/dimension limits | Sometimes | Dimension exceeds limit, file I/O errors (disk full, memory pressure) |
| **InternalBug** | Library bugs (should not happen) | No | Internal panic, unexpected state |

## Panic Policy

All codec entry points (decode/encode/ICC embedding) execute inside a unified
panic guard. Any panic raised by third-party libraries such as mozjpeg,
libavif, img-parts, or the `image` crate is caught and converted to
`LazyImageError::InternalPanic` before it reaches JavaScript. This guarantees:

- The Node.js process never aborts due to codec panics.
- Callers consistently receive an `InternalBug` category when a dependency
  misbehaves.
- The `run_with_panic_policy()` helper enforces this rule, so new codec paths
  must wrap their core logic with it.

If you encounter an `InternalBug` from lazy-image, please file an issue with
the panic message so we can reproduce and patch the underlying codec.

### Using Error Categories in JavaScript

Errors from lazy-image include category information in the `error.code` property (e.g., `"LAZY_IMAGE_USER_ERROR"`) or `error.category` property (ErrorCategory enum value). Use the `getErrorCategory()` helper function to extract the category:

```javascript
const { ImageEngine, ErrorCategory, getErrorCategory } = require('@alberteinshutoin/lazy-image');

try {
  await ImageEngine.from(buffer)
    .rotate(45)  // Invalid rotation angle
    .toBuffer('jpeg', 80);
} catch (err) {
  const category = getErrorCategory(err);
  
  if (category === ErrorCategory.UserError) {
    // User can fix this - invalid input
    console.log('Please check your input:', err.message);
  } else if (category === ErrorCategory.CodecError) {
    // Format/encoding issue - may need to convert or fix the image
    console.log('Image format issue:', err.message);
  } else if (category === ErrorCategory.ResourceLimit) {
    // Resource constraint - may need to resize or free up resources
    console.log('Resource limit reached:', err.message);
  } else if (category === ErrorCategory.InternalBug) {
    // Library bug - should report to maintainers
    console.error('Internal error - please report:', err.message);
  } else {
    // category is null - error.code not set (legacy error or not from lazy-image)
    console.log('Error without category:', err.message);
  }
}
```

All thrown errors also include:
- `error.errorCode` — fine-grained classification such as `E200`
- `error.recoveryHint` — short guidance string describing how to fix the issue

### Batch Processing Error Metadata

`ImageEngine.processBatch()` returns an array of `BatchResult` objects. Each failed entry now includes:

- `error` – Human-readable message with source context
- `errorCode` – Fine-grained code like `E100`, `E200`, etc.
- `errorCategory` – `ErrorCategory` enum value

This makes it possible to inspect per-file failures without parsing strings.

**Note**: The `error.code` and `error.category` properties are set when errors are created using `create_napi_error_with_code()`. All error paths in lazy-image now use this function, so `getErrorCategory()` will return the appropriate category for all lazy-image errors.

### Error Category Classification

**UserError** - Invalid input that the user can fix:
- `FileNotFound` - File path doesn't exist
- `InvalidCropBounds` - Crop bounds exceed image dimensions
- `InvalidRotationAngle` - Unsupported rotation angle
- `InvalidResizeDimensions` - Invalid resize parameters
- `InvalidPreset` - Unknown preset name
- `InvalidFirewallPolicy` - Unknown policy passed to `sanitize()`
- `SourceConsumed` - Image source already used

**CodecError** - Format/encoding issues:
- `UnsupportedFormat` - Format not supported
- `DecodeFailed` - Failed to decode image
- `CorruptedImage` - Image data is corrupted
- `EncodeFailed` - Failed to encode image
- `UnsupportedColorSpace` - Color space not supported
- `ResizeFailed` - Resize operation failed (processing error, classified as codec error)

**ResourceLimit** - Resource constraints:
- `DimensionExceedsLimit` - Image dimensions too large
- `PixelCountExceedsLimit` - Too many pixels
- `FileReadFailed` - File read failed (often due to resource constraints like disk full, memory pressure)
- `MmapFailed` - Memory mapping failed (often due to resource constraints)
- `FileWriteFailed` - File write failed (often due to resource constraints like disk full)
- `FirewallViolation` - Image Firewall blocked the input (bytes, pixels, metadata, or timeout)

**InternalBug** - Library bugs:
- `InternalPanic` - Unexpected internal error
- `Generic` - Generic internal error

## Error Code Categories

| Category | Range | Description |
|----------|-------|-------------|
| **E1xx** | 100-199 | Input Errors - Issues with input files or data |
| **E2xx** | 200-299 | Processing Errors - Issues during image processing operations |
| **E3xx** | 300-399 | Output Errors - Issues when writing or encoding output |
| **E4xx** | 400-499 | Configuration Errors - Invalid parameters or settings |
| **E9xx** | 900-999 | Internal Errors - Unexpected internal state or bugs |

## Error Codes

### Input Errors (E1xx)

#### E100: File Not Found
**Recoverable**: Yes

The specified file path does not exist.

**Common causes:**
- Incorrect file path
- File was moved or deleted
- Typo in filename

**How to fix:**
- Verify the file path is correct
- Check file permissions
- Ensure the file exists before processing

---

#### E101: File Read Failed
**Recoverable**: Yes

An I/O error occurred while reading the file.

**Common causes:**
- Insufficient file permissions
- Disk I/O errors
- File is locked by another process

**How to fix:**
- Check file permissions
- Ensure disk space is available
- Close other processes using the file

---

#### E110: Invalid Image Format
**Recoverable**: No

The file format is not recognized or is invalid.

**Common causes:**
- File is corrupted
- File is not an image
- File extension doesn't match content

**How to fix:**
- Verify the file is a valid image
- Check if the file is corrupted
- Try opening the file in an image viewer

---

#### E111: Unsupported Image Format
**Recoverable**: No

The image format is recognized but not supported by lazy-image.

**Supported formats:**
- JPEG/JPG
- PNG
- WebP

**How to fix:**
- Convert the image to a supported format
- Use a different image processing library for unsupported formats

---

#### E120: Image Too Large
**Recoverable**: No

The image exceeds size limits (file size or memory constraints).

**How to fix:**
- Resize or compress the image before processing
- Process the image in smaller chunks
- Increase available memory

---

#### E121: Dimension Exceeds Limit
**Recoverable**: Yes

Image width or height exceeds the maximum allowed dimension.

**How to fix:**
- Resize the image to fit within limits
- Check the maximum dimension limits in your configuration

---

#### E122: Pixel Count Exceeds Limit
**Recoverable**: Yes

Total pixel count (width × height) exceeds the maximum allowed.

**How to fix:**
- Resize the image to reduce pixel count
- Process smaller images or use batch processing

---

#### E130: Corrupted Image Data
**Recoverable**: No

The image file is corrupted or contains invalid data.

**Common causes:**
- File transfer errors
- Disk corruption
- Incomplete file download

**How to fix:**
- Re-download or re-copy the file
- Use image repair tools
- Restore from backup

---

#### E131: Failed to Decode Image
**Recoverable**: No

An error occurred during image decoding (format-specific issue).

**Common causes:**
- Corrupted image data
- Unsupported format variant
- Encoding issues

**How to fix:**
- Verify the file is a valid image
- Try opening in another image viewer
- Re-encode the image

---

### Processing Errors (E2xx)

#### E200: Invalid Crop Bounds
**Recoverable**: Yes

Crop coordinates exceed image dimensions.

**How to fix:**
- Ensure crop coordinates are within image bounds
- Check that `x + width ≤ image_width` and `y + height ≤ image_height`
- Use `inspect()` to get image dimensions first

---

#### E201: Invalid Rotation Angle
**Recoverable**: Yes

Rotation angle is not a multiple of 90 degrees.

**Supported angles:**
- 0, 90, 180, 270 degrees
- Negative equivalents: -90, -180, -270

**How to fix:**
- Use only 90-degree increments
- For arbitrary angles, use a different library

---

#### E202: Invalid Resize Dimensions
**Recoverable**: Yes

Resize dimensions are invalid (e.g., both width and height are None).

**How to fix:**
- Provide at least one valid dimension (width or height)
- Ensure dimensions are positive integers
- Use `None` for one dimension to maintain aspect ratio

---

#### E210: Unsupported Color Space
**Recoverable**: No

The requested color space conversion is not supported.

**How to fix:**
- Use a supported color space (sRGB, Display P3, Adobe RGB)
- Check color space support in the documentation

---

#### E299: Operation Failed
**Recoverable**: Depends

A general processing operation failed.

**How to fix:**
- Check the error message for specific details
- Verify input parameters
- Try a different approach

---

### Output Errors (E3xx)

#### E300: Failed to Encode Image
**Recoverable**: No

An error occurred during image encoding (format-specific issue).

**Common causes:**
- Invalid encoding parameters
- Format-specific limitations
- Memory constraints

**How to fix:**
- Check encoding parameters (quality, format)
- Try a different output format
- Reduce image size before encoding

---

#### E301: Failed to Write File
**Recoverable**: Yes

An I/O error occurred while writing the output file.

**Common causes:**
- Insufficient disk space
- Write permissions denied
- Disk I/O errors

**How to fix:**
- Check disk space
- Verify write permissions for output directory
- Ensure output path is valid

---

#### E302: Output Path Invalid
**Recoverable**: Yes

The output file path is invalid or inaccessible.

**How to fix:**
- Verify the output directory exists
- Check path format (no invalid characters)
- Ensure write permissions for the directory

---

### Configuration Errors (E4xx)

#### E400: Invalid Quality Value
**Recoverable**: Yes

Quality parameter is out of valid range (typically 1-100).

**How to fix:**
- Use a quality value between 1 and 100
- Check format-specific quality requirements

---

#### E401: Invalid Preset Name
**Recoverable**: Yes

The specified preset name is not recognized.

**Available presets:**
- `thumbnail` - 150x150, WebP quality 75
- `avatar` - 200x200, WebP quality 80
- `hero` - 1920 width, JPEG quality 85
- `social` - 1200x630, JPEG quality 80

**How to fix:**
- Use one of the available preset names
- Check preset names are case-insensitive

---

### Internal Errors (E9xx)

#### E900: Source Already Consumed
**Recoverable**: Yes

Image source has already been consumed and cannot be reused.

**How to fix:**
- Use `clone()` for multi-output scenarios
- Create a new `ImageEngine` instance for each output

---

#### E901: Internal Panic
**Recoverable**: No

An unexpected internal error occurred (likely a bug).

**How to fix:**
- Report this as a bug with:
  - Error code (E901)
  - Input image details
  - Steps to reproduce
  - Full error message

---

#### E999: Unexpected State
**Recoverable**: No

The library is in an unexpected internal state.

**How to fix:**
- Report this as a bug with:
  - Error code (E999)
  - Operation that triggered the error
  - Full error message and stack trace

---

## Using Error Codes in Code

### Rust

```rust
use lazy_image::error::{ErrorCode, LazyImageError};

match result {
    Ok(data) => println!("Success!"),
    Err(err) => {
        match err.code() {
            ErrorCode::FileNotFound => {
                eprintln!("File not found: {}", err);
            }
            ErrorCode::InvalidCropBounds => {
                eprintln!("Invalid crop bounds: {}", err);
            }
            _ => {
                eprintln!("Error {}: {}", err.code(), err);
            }
        }
    }
}
```

### JavaScript/TypeScript

```typescript
import { ImageEngine, ErrorCode } from '@alberteinshutoin/lazy-image';

try {
    const result = await ImageEngine.fromFile('input.jpg')
        .resize(800)
        .toBuffer('jpeg', 85);
} catch (error) {
    // Error message contains error code like "[E100] File not found: ..."
    const errorCode = error.message.match(/\[E\d+\]/)?.[0];
    
    if (errorCode === '[E100]') {
        console.error('File not found');
    } else if (errorCode === '[E200]') {
        console.error('Invalid crop bounds');
    } else {
        console.error('Error:', error.message);
    }
}
```

## Error Recovery

Errors marked as **Recoverable** can be handled programmatically:

```rust
if err.code().is_recoverable() {
    // User can fix this - retry with corrected input
    retry_with_corrected_input();
} else {
    // Non-recoverable - log and report
    log_error(err);
    report_bug(err);
}
```

## Best Practices

1. **Always check error codes** - Don't rely solely on error messages
2. **Handle recoverable errors** - Provide user-friendly messages and retry options
3. **Log non-recoverable errors** - Report bugs with full context
4. **Use error codes for monitoring** - Track error rates by code
5. **Document error handling** - Make error handling part of your API documentation
