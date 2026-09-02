'use strict';

const fs = require('node:fs');

// ---------------------------------------------------------------------------
// Helper functions extracted from the postbuild-appended JS block.
// These are proper source files now instead of string-concatenated code.
// ---------------------------------------------------------------------------

const DEFAULT_QUALITY_BY_FORMAT = {
  jpeg: 85,
  jpg: 85,
  webp: 80,
  avif: 60,
};

const TARGET_BYTES_SUPPORTED_FORMATS = new Set(['jpeg', 'jpg', 'webp', 'avif']);
const DEFAULT_TARGET_BYTES_MIN_QUALITY = 30;

const ENCODE_PROFILE_CONFIG = {
  'size-first': {
    jpeg: { qualityDelta: -8, fastMode: false },
    webp: { qualityDelta: -8, fastMode: false },
    avif: { qualityDelta: -6, fastMode: false },
    png: { qualityDelta: 0, fastMode: false },
  },
  balanced: {
    jpeg: { qualityDelta: 0, fastMode: false },
    webp: { qualityDelta: 0, fastMode: false },
    avif: { qualityDelta: 0, fastMode: false },
    png: { qualityDelta: 0, fastMode: false },
  },
  'speed-first': {
    jpeg: { qualityDelta: -10, fastMode: true },
    webp: { qualityDelta: -10, fastMode: false },
    avif: { qualityDelta: -10, fastMode: false },
    png: { qualityDelta: 0, fastMode: false },
  },
};

// ---------------------------------------------------------------------------
// Pure helpers (no dependency on nativeBinding)
// ---------------------------------------------------------------------------

/** Clamp a value to a valid quality integer (1-100), or return undefined. */
function clampQuality(v) {
  const n = Number(v);
  if (!Number.isFinite(n)) return undefined;
  return Math.max(1, Math.min(100, Math.round(n)));
}

/** Parse a target-bytes quality integer without silently clamping invalid ranges. */
function parseTargetQuality(v) {
  const n = Number(v);
  if (!Number.isFinite(n)) return undefined;
  return Math.round(n);
}

/** Validate and normalize target-bytes options for binary-search encoding. */
function normalizeTargetBytesOptions(format, options = {}) {
  const normalizedFormat = String(format || '').toLowerCase();
  if (!TARGET_BYTES_SUPPORTED_FORMATS.has(normalizedFormat)) {
    throw new Error(`Target-bytes mode supports jpeg/webp/avif only: ${format}`);
  }

  const rawBudget = options.targetBytes ?? options.maxBytes;
  const targetBytes = Number(rawBudget);
  if (!Number.isFinite(targetBytes) || targetBytes <= 0) {
    throw new Error('targetBytes must be a positive number');
  }
  if (targetBytes > 0xffff_ffff) {
    throw new Error('targetBytes must not exceed the current u32 limit (4294967295)');
  }

  const minQuality = parseTargetQuality(options.minQuality ?? DEFAULT_TARGET_BYTES_MIN_QUALITY);
  const maxQuality = parseTargetQuality(options.maxQuality ?? 100);
  if (
    minQuality == null ||
    maxQuality == null ||
    minQuality <= 0 ||
    maxQuality <= 0 ||
    minQuality > 100 ||
    maxQuality > 100
  ) {
    throw new Error('minQuality/maxQuality must be integers between 1 and 100');
  }
  if (minQuality > maxQuality) {
    throw new Error('minQuality must be less than or equal to maxQuality');
  }

  if (options.strict !== undefined && typeof options.strict !== 'boolean') {
    const error = new TypeError('strict must be a boolean when provided');
    error.code = 'LAZY_IMAGE_USER_ERROR';
    error.errorCode = 'E400';
    error.errorCategory = 'UserError';
    error.category = 0;
    error.recoveryHint = 'Set strict to true or false.';
    throw error;
  }

  const policy = options.strict === true
    ? 'strict'
    : (options.qualityFloorPolicy == null ? 'best-effort' : String(options.qualityFloorPolicy));
  if (!['best-effort', 'strict'].includes(policy)) {
    throw new Error(`Unsupported qualityFloorPolicy: ${options.qualityFloorPolicy}`);
  }

  return {
    format: normalizedFormat === 'jpg' ? 'jpeg' : normalizedFormat,
    targetBytes: Math.round(targetBytes),
    minQuality,
    maxQuality,
    fastMode: Boolean(options.fastMode),
    qualityFloorPolicy: policy,
  };
}

/**
 * Shared target-bytes search used by toBufferTargetBytes and toFileTargetBytes.
 * Implementation now requires a native binding that exposes
 * `toBufferTargetBytesNative`; when it is missing we fail fast with an
 * actionable message to force a version-aligned reinstall.
 *
 * @param {object} engine  – ImageEngine instance (must have toBufferTargetBytesNative)
 * @param {string} format  – output format (jpeg/webp/avif)
 * @param {object} options – TargetBytesOptions
 * @returns {Promise<{data: Buffer, quality: number, bytesOut: number, budgetMet: boolean, targetBytes: number, metrics: object}>}
 */
async function runTargetBytesSearch(engine, format, options) {
  const config = normalizeTargetBytesOptions(format, options);

  if (typeof engine.toBufferTargetBytesNative === 'function') {
    const result = await engine.toBufferTargetBytesNative(
      config.format,
      config.targetBytes,
      config.minQuality,
      config.maxQuality,
      config.fastMode,
      config.qualityFloorPolicy === 'strict',
    );
    return {
      data: result.data,
      quality: result.quality,
      bytesOut: result.bytesOut,
      budgetMet: result.budgetMet,
      targetBytes: result.targetBytes,
      metrics: result.metrics,
    };
  }

  throw new Error(
    'toBufferTargetBytes requires a native binding with toBufferTargetBytesNative. ' +
      'Reinstall @alberteinshutoin/lazy-image so the JS and native versions match.',
  );
}

/** Run target-bytes search and atomic file output without creating a JS Buffer. */
async function runTargetBytesFileSearch(engine, path, format, options) {
  const config = normalizeTargetBytesOptions(format, options);
  if (typeof engine.toFileTargetBytesNative !== 'function') {
    throw new Error(
      'toFileTargetBytes requires a native binding with toFileTargetBytesNative. ' +
        'Reinstall @alberteinshutoin/lazy-image so the JS and native versions match.',
    );
  }

  return engine.toFileTargetBytesNative(
    path,
    config.format,
    config.targetBytes,
    config.minQuality,
    config.maxQuality,
    config.fastMode,
    config.qualityFloorPolicy === 'strict',
  );
}

/** Resolve an encode-profile name into concrete quality/fastMode settings. */
function resolveEncodeProfile(format, profile = 'balanced', quality) {
  const normalizedFormat = String(format || '').toLowerCase();
  if (!['jpeg', 'jpg', 'png', 'webp', 'avif'].includes(normalizedFormat)) {
    throw new Error(`Unsupported format for profile: ${format}`);
  }

  const normalizedProfile = String(profile || 'balanced').toLowerCase();
  if (!Object.hasOwn(ENCODE_PROFILE_CONFIG, normalizedProfile)) {
    throw new Error(`Unsupported encode profile: ${profile}`);
  }

  const profileConfig = ENCODE_PROFILE_CONFIG[normalizedProfile];
  const formatKey = normalizedFormat === 'jpg' ? 'jpeg' : normalizedFormat;
  // PNG is lossless — the native encoder now rejects an explicit quality for
  // PNG (E400). Never resolve/forward a quality for PNG, otherwise a profile
  // would pass one through to toBuffer('png', q) and trigger a confusing error.
  const isPng = formatKey === 'png';
  const baseQuality = clampQuality(quality) ?? DEFAULT_QUALITY_BY_FORMAT[formatKey];
  const delta = (profileConfig[formatKey] || profileConfig.png).qualityDelta;
  const resolvedQuality =
    isPng || baseQuality == null ? undefined : clampQuality(baseQuality + delta);
  const fastMode = Boolean((profileConfig[formatKey] || profileConfig.png).fastMode);

  return {
    format: normalizedFormat,
    quality: resolvedQuality,
    fastMode,
  };
}

/** Validate and normalize options shared by responsive output helpers. */
function normalizeResponsiveOptions(options = {}) {
  if (options === null || typeof options !== 'object' || Array.isArray(options)) {
    throw new TypeError('responsive options must be an object');
  }

  const rawWidths = options.widths;
  if (!Array.isArray(rawWidths) || rawWidths.length === 0) {
    throw new Error('widths must be a non-empty array of positive integers');
  }

  const widths = [...new Set(rawWidths.map((width) => {
    const normalized = Number(width);
    if (!Number.isInteger(normalized) || normalized <= 0) {
      throw new Error('widths must be a non-empty array of positive integers');
    }
    return normalized;
  }))];

  if (options.format == null) {
    throw new Error('format is required for responsive output');
  }

  return {
    widths,
    format: String(options.format).toLowerCase(),
    quality: options.quality,
    fastMode: Boolean(options.fastMode),
  };
}

// ---------------------------------------------------------------------------
// getErrorCategory — needs ErrorCategory / ErrorCode from the native binding.
// Accepts them as an argument so this module stays decoupled from index.js.
// ---------------------------------------------------------------------------

function createGetErrorCategory(ErrorCategory, ErrorCode) {
  return function getErrorCategory(err) {
    if (!err) return null;

    if (typeof err.errorCategory === 'number') return err.errorCategory;
    if (typeof err.category === 'number') return err.category;

    if (typeof err.code === 'string') {
      switch (err.code) {
        case 'LAZY_IMAGE_USER_ERROR':
          return ErrorCategory.UserError;
        case 'LAZY_IMAGE_CODEC_ERROR':
          return ErrorCategory.CodecError;
        case 'LAZY_IMAGE_RESOURCE_LIMIT':
          return ErrorCategory.ResourceLimit;
        case 'LAZY_IMAGE_INTERNAL_BUG':
          return ErrorCategory.InternalBug;
        default:
          break;
      }
    }

    let numericCode = null;
    if (typeof err.errorCode === 'string' && /^E[0-9]{3}$/.test(err.errorCode)) {
      numericCode = parseInt(err.errorCode.slice(1), 10);
    } else if (typeof err.errorCode === 'number') {
      numericCode = err.errorCode;
    }

    if (numericCode != null && Number.isFinite(numericCode)) {
      switch (numericCode) {
        case ErrorCode.FileNotFound:
        case ErrorCode.InvalidCropBounds:
        case ErrorCode.InvalidCropDimensions:
        case ErrorCode.InvalidRotationAngle:
        case ErrorCode.InvalidResizeDimensions:
        case ErrorCode.InvalidResizeFit:
        case ErrorCode.InvalidArgument:
        case ErrorCode.InvalidPreset:
        case ErrorCode.InvalidFirewallPolicy:
        case ErrorCode.SourceConsumed:
          return ErrorCategory.UserError;
        case ErrorCode.UnsupportedFormat:
        case ErrorCode.DecodeFailed:
        case ErrorCode.CorruptedImage:
        case ErrorCode.EncodeFailed:
        case ErrorCode.UnsupportedColorSpace:
        case ErrorCode.ResizeFailed:
          return ErrorCategory.CodecError;
        case ErrorCode.DimensionExceedsLimit:
        case ErrorCode.PixelCountExceedsLimit:
        case ErrorCode.FileReadFailed:
        case ErrorCode.MmapFailed:
        case ErrorCode.FileWriteFailed:
        case ErrorCode.FirewallViolation:
          return ErrorCategory.ResourceLimit;
        case ErrorCode.InternalPanic:
        case ErrorCode.Generic:
          return ErrorCategory.InternalBug;
        default:
          break;
      }

      const bucket = Math.floor(numericCode / 100);
      if (bucket === 1 || bucket === 2 || bucket === 4) return ErrorCategory.UserError;
      if (bucket === 3) return ErrorCategory.ResourceLimit;
      if (bucket === 9) return ErrorCategory.InternalBug;
    }

    return null;
  };
}

// ---------------------------------------------------------------------------
// processBatchChunked — chunk-level progress and abort around native batches.
// ---------------------------------------------------------------------------

function createAbortError() {
  if (typeof DOMException === 'function') {
    return new DOMException('processBatchChunked was aborted', 'AbortError');
  }
  const error = new Error('processBatchChunked was aborted');
  error.name = 'AbortError';
  return error;
}

function isReadableInputFile(input) {
  if (typeof input !== 'string') return false;
  try {
    fs.accessSync(input, fs.constants.R_OK);
    return fs.statSync(input).isFile();
  } catch {
    return false;
  }
}

function createInputReadFailure(input) {
  return {
    source: input,
    success: false,
    error: `[E100] ${input}: failed to read input file`,
    errorCode: 'E100',
    errorCategory: 'UserError',
  };
}

function createProcessBatchChunked(ImageEngine) {
  return function processBatchChunked(inputs, outputDir, options = {}) {
    if (!Array.isArray(inputs)) {
      throw new Error('inputs must be an array of file paths');
    }
    if (!ImageEngine || typeof ImageEngine.fromPath !== 'function') {
      throw new Error('processBatchChunked requires ImageEngine.fromPath');
    }

    const { chunkSize = 16, onProgress, signal, ...batchOptions } = options;
    if (!Number.isInteger(chunkSize) || chunkSize < 1) {
      throw new Error('chunkSize must be a positive integer');
    }

    const throwIfAborted = () => {
      if (signal?.aborted) {
        throw createAbortError();
      }
    };

    const processChunk = (chunk) => {
      if (chunk.length === 0) {
        return Promise.resolve([]);
      }
      const bootstrapInput = chunk.find(isReadableInputFile);
      if (!bootstrapInput) {
        return Promise.resolve(chunk.map(createInputReadFailure));
      }
      return ImageEngine.fromPath(bootstrapInput).processBatch(chunk, outputDir, batchOptions);
    };

    const reportProgress = (results, chunkResults, failed) => {
      if (!onProgress) return;
      try {
        onProgress({
          completed: results.length,
          total: inputs.length,
          lastChunk: chunkResults,
          failed,
        });
      } catch (err) {
        console.error('processBatchChunked onProgress callback threw:', err);
      }
    };

    if (inputs.length <= chunkSize) {
      return (async () => {
        throwIfAborted();
        const chunkResults = await processChunk(inputs);
        const failed = chunkResults.filter((result) => !result.success).length;
        reportProgress(chunkResults, chunkResults, failed);
        return chunkResults;
      })();
    }

    return (async () => {
      const results = [];
      let failed = 0;
      for (let i = 0; i < inputs.length; i += chunkSize) {
        throwIfAborted();
        const chunk = inputs.slice(i, i + chunkSize);
        const chunkResults = await processChunk(chunk);
        results.push(...chunkResults);
        failed += chunkResults.filter((result) => !result.success).length;
        reportProgress(results, chunkResults, failed);
      }
      return results;
    })();
  };
}

// ---------------------------------------------------------------------------
// Prototype method attachment — called with the ImageEngine class
// ---------------------------------------------------------------------------

/** Attach profile and target-bytes convenience methods to ImageEngine.prototype. */
function attachPrototypeMethods(ImageEngine) {
  if (!ImageEngine || !ImageEngine.prototype) return;
  const p = ImageEngine.prototype;

  p.toBufferProfile = function toBufferProfile(format, profile = 'balanced', quality) {
    const resolved = resolveEncodeProfile(format, profile, quality);
    return this.toBuffer(resolved.format, resolved.quality, resolved.fastMode);
  };

  p.toBufferWithMetricsProfile = function toBufferWithMetricsProfile(format, profile = 'balanced', quality) {
    const resolved = resolveEncodeProfile(format, profile, quality);
    return this.toBufferWithMetrics(resolved.format, resolved.quality, resolved.fastMode);
  };

  p.toFileProfile = function toFileProfile(path, format, profile = 'balanced', quality) {
    const resolved = resolveEncodeProfile(format, profile, quality);
    return this.toFile(path, resolved.format, resolved.quality, resolved.fastMode);
  };

  p.toBufferTargetBytes = async function toBufferTargetBytes(format, options) {
    const result = await runTargetBytesSearch(this, format, options);
    return {
      data: result.data,
      quality: result.quality,
      bytesOut: result.bytesOut,
      budgetMet: result.budgetMet,
      targetBytes: result.targetBytes,
      metrics: result.metrics,
    };
  };

  p.toFileTargetBytes = async function toFileTargetBytes(path, format, options) {
    return runTargetBytesFileSearch(this, path, format, options);
  };

  p.toResponsiveSet = function toResponsiveSet(options) {
    const { widths, format, quality, fastMode } = normalizeResponsiveOptions(options);
    return Promise.all(widths.map(async (width) => {
      const data = await this.clone().resize(width).toBuffer(format, quality, fastMode);
      return { width, data, bytes: data.length };
    }));
  };

  p.toFilesResponsive = function toFilesResponsive(pattern, options) {
    if (typeof pattern !== 'string' || !pattern.includes('{width}')) {
      throw new Error("pattern must contain a {width} placeholder, e.g. 'out-{width}.webp'");
    }
    const { widths, format, quality, fastMode } = normalizeResponsiveOptions(options);
    return Promise.all(widths.map(async (width) => {
      const filePath = pattern.replaceAll('{width}', String(width));
      const bytesWritten = await this.clone().resize(width).toFile(filePath, format, quality, fastMode);
      return { width, path: filePath, bytesWritten };
    })).then((files) => ({
      files,
      srcset: files.map((file) => `${file.path} ${file.width}w`).join(', '),
    }));
  };
}

// ---------------------------------------------------------------------------
// Exports
// ---------------------------------------------------------------------------

module.exports = {
  DEFAULT_QUALITY_BY_FORMAT,
  TARGET_BYTES_SUPPORTED_FORMATS,
  ENCODE_PROFILE_CONFIG,
  clampQuality,
  parseTargetQuality,
  normalizeTargetBytesOptions,
  runTargetBytesSearch,
  runTargetBytesFileSearch,
  resolveEncodeProfile,
  normalizeResponsiveOptions,
  createGetErrorCategory,
  createProcessBatchChunked,
  attachPrototypeMethods,
};
