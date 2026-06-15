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
 *   node examples/error-handling.mjs --self-test
 */

import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import {
  fixturePath,
  hasSelfTestFlag,
  loadLazyImage,
  makeTempDir,
  removeDir,
} from './_load.mjs';

const {
  ImageEngine,
  ErrorCategory,
  getErrorCategory,
  inspectFile,
} = loadLazyImage();

if (hasSelfTestFlag()) {
  await runSelfTest();
} else {
  await runDemo();
}

async function runSelfTest() {
  const tmp = makeTempDir('lazy-image-errors-');
  const input = fixturePath('test_100KB_1057x1057.jpg');

  try {
    try {
      await ImageEngine.fromPath(path.join(tmp, 'missing.jpg'))
        .resize(800)
        .toBuffer('jpeg', 85);
      assert.fail('missing input should throw');
    } catch (err) {
      assert.equal(getErrorCategory(err), ErrorCategory.UserError);
      assert(err.errorCode);
    }

    const outputPath = path.join(tmp, 'safe.webp');
    const bytesWritten = await safeProcess(input, outputPath);
    assert(bytesWritten > 0);
    assert(fs.existsSync(outputPath));

    const buffer = fs.readFileSync(input);
    try {
      await ImageEngine.from(buffer)
        .limits({ maxPixels: 1 })
        .toBuffer('webp', 80);
      assert.fail('pixel limit should throw');
    } catch (err) {
      assert.equal(getErrorCategory(err), ErrorCategory.ResourceLimit);
      assert.equal(err.errorCode, 'E123');
    }
  } finally {
    removeDir(tmp);
  }
}

async function runDemo() {
  // 1. Category-based error recovery
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

    // Fine-grained code and recovery hint are also available.
    console.log('Error code:', err.errorCode);
    console.log('Recovery hint:', err.recoveryHint);
  }

  // 2. Validate before processing
  await safeProcess('input.jpg', 'output.webp');

  // 3. Image Firewall for untrusted uploads
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
}

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
