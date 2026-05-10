/**
 * Error handling examples for lazy-image.
 *
 * lazy-image uses a 4-tier error taxonomy:
 *   - UserError: invalid input, recoverable by the caller
 *   - CodecError: format/encoding issues
 *   - ResourceLimit: memory/dimension limits
 *   - InternalBug: library bugs (please report)
 *
 * Run:
 *   node examples/error-handling.mjs
 */

import {
  ImageEngine,
  ErrorCategory,
  getErrorCategory,
  inspectFile,
} from '@alberteinshutoin/lazy-image';

// ── 1. Category-based error recovery ───────────────────────────────
try {
  await ImageEngine.fromPath('missing.jpg')
    .resize(800)
    .toBuffer('jpeg', 85);
} catch (err) {
  const category = getErrorCategory(err);

  switch (category) {
    case ErrorCategory.UserError:
      console.log('Fix your input:', err.message);
      break;
    case ErrorCategory.CodecError:
      console.log('Image format issue:', err.message);
      break;
    case ErrorCategory.ResourceLimit:
      console.log('Resource limit hit:', err.message);
      break;
    case ErrorCategory.InternalBug:
      console.error('Please report this bug:', err.message);
      break;
    default:
      console.log('Unknown error:', err.message);
  }

  // Fine-grained code and recovery hint are also available
  console.log('Error code:', err.errorCode);
  console.log('Recovery hint:', err.recoveryHint);
}

// ── 2. Validate before processing ──────────────────────────────────
function safeProcess(inputPath, outputPath) {
  const MAX_DIMENSION = 10000;

  const meta = inspectFile(inputPath);

  if (meta.width > MAX_DIMENSION || meta.height > MAX_DIMENSION) {
    throw new Error(`Image too large: ${meta.width}x${meta.height}`);
  }

  return ImageEngine.fromPath(inputPath)
    .resize(800)
    .toFile(outputPath, 'webp', 80);
}

// ── 3. Image Firewall for untrusted uploads ────────────────────────
try {
  await ImageEngine.fromPath('untrusted-upload.jpg')
    .sanitize({ policy: 'strict' })
    .resize(1200)
    .toBuffer('webp', 80);
} catch (err) {
  const category = getErrorCategory(err);
  if (category === ErrorCategory.ResourceLimit) {
    console.log('Upload rejected by firewall:', err.message);
  }
}
