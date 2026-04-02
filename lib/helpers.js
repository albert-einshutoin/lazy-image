'use strict';

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

function clampQuality(v) {
  const n = Number(v);
  if (!Number.isFinite(n)) return undefined;
  return Math.max(0, Math.min(100, Math.round(n)));
}

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

  const minQuality = clampQuality(options.minQuality ?? DEFAULT_TARGET_BYTES_MIN_QUALITY);
  const maxQuality = clampQuality(options.maxQuality ?? 100);
  if (minQuality == null || maxQuality == null || minQuality <= 0 || maxQuality <= 0) {
    throw new Error('minQuality/maxQuality must be integers between 1 and 100');
  }
  if (minQuality > maxQuality) {
    throw new Error('minQuality must be less than or equal to maxQuality');
  }

  const policy = options.qualityFloorPolicy == null ? 'best-effort' : String(options.qualityFloorPolicy);
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

async function encodeWithinByteBudget(engine, format, options) {
  const config = normalizeTargetBytesOptions(format, options);
  let low = config.minQuality;
  let high = config.maxQuality;
  let bestUnder = null;
  let bestOver = null;

  while (low <= high) {
    const quality = Math.floor((low + high) / 2);
    const result = await engine.toBufferWithMetrics(config.format, quality, config.fastMode);
    const candidate = {
      data: result.data,
      metrics: result.metrics,
      quality,
      bytesOut: result.metrics?.bytesOut ?? result.data.length,
    };

    if (candidate.bytesOut <= config.targetBytes) {
      bestUnder = candidate;
      low = quality + 1;
    } else {
      if (
        !bestOver ||
        candidate.bytesOut < bestOver.bytesOut ||
        (candidate.bytesOut === bestOver.bytesOut && quality > bestOver.quality)
      ) {
        bestOver = candidate;
      }
      high = quality - 1;
    }
  }

  const chosen = bestUnder || bestOver;
  if (!chosen) {
    throw new Error('Failed to encode any candidate while searching for target bytes');
  }

  const budgetMet = chosen.bytesOut <= config.targetBytes;
  if (!budgetMet && config.qualityFloorPolicy === 'strict') {
    throw new Error(`Unable to meet targetBytes=${config.targetBytes} within quality range ${config.minQuality}-${config.maxQuality}`);
  }

  return {
    ...chosen,
    budgetMet,
    targetBytes: config.targetBytes,
    format: config.format,
    fastMode: config.fastMode,
    qualityFloorPolicy: config.qualityFloorPolicy,
  };
}

function resolveEncodeProfile(format, profile = 'balanced', quality) {
  const normalizedFormat = String(format || '').toLowerCase();
  if (!['jpeg', 'jpg', 'png', 'webp', 'avif'].includes(normalizedFormat)) {
    throw new Error(`Unsupported format for profile: ${format}`);
  }

  const normalizedProfile = String(profile || 'balanced').toLowerCase();
  if (!Object.prototype.hasOwnProperty.call(ENCODE_PROFILE_CONFIG, normalizedProfile)) {
    throw new Error(`Unsupported encode profile: ${profile}`);
  }

  const profileConfig = ENCODE_PROFILE_CONFIG[normalizedProfile];
  const formatKey = normalizedFormat === 'jpg' ? 'jpeg' : normalizedFormat;
  const baseQuality = clampQuality(quality) ?? DEFAULT_QUALITY_BY_FORMAT[formatKey];
  const delta = (profileConfig[formatKey] || profileConfig.png).qualityDelta;
  const resolvedQuality = baseQuality == null ? undefined : clampQuality(baseQuality + delta);
  const fastMode = Boolean((profileConfig[formatKey] || profileConfig.png).fastMode);

  return {
    format: normalizedFormat,
    quality: resolvedQuality,
    fastMode,
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
// Prototype method attachment — called with the ImageEngine class
// ---------------------------------------------------------------------------

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
    // Use Rust-native binary search when available (eliminates ~7 JS↔NAPI round-trips)
    if (typeof this.toBufferTargetBytesNative === 'function') {
      const config = normalizeTargetBytesOptions(format, options);
      const result = await this.toBufferTargetBytesNative(
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

    // Fallback: JS-side binary search (for older native bindings)
    const result = await encodeWithinByteBudget(this, format, options);
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
    // Use Rust-native binary search when available.
    // Write the buffer produced by toBufferTargetBytesNative directly to
    // disk instead of re-encoding — lossy codecs are not byte-deterministic
    // between two calls, so a second encode could exceed the budget.
    if (typeof this.toBufferTargetBytesNative === 'function') {
      const fs = require('fs/promises');
      const config = normalizeTargetBytesOptions(format, options);
      const result = await this.toBufferTargetBytesNative(
        config.format,
        config.targetBytes,
        config.minQuality,
        config.maxQuality,
        config.fastMode,
        config.qualityFloorPolicy === 'strict',
      );
      await fs.writeFile(path, result.data);
      return {
        bytesWritten: result.data.length,
        quality: result.quality,
        budgetMet: result.budgetMet,
        targetBytes: config.targetBytes,
        metrics: result.metrics,
      };
    }

    // Fallback: JS-side binary search (for older native bindings that lack
    // toBufferTargetBytesNative).  toFileWithMetrics may also be missing in
    // the same scenario, so fall back to toFile when necessary.
    const result = await encodeWithinByteBudget(this, format, options);
    if (typeof this.toFileWithMetrics === 'function') {
      const written = await this.toFileWithMetrics(path, result.format, result.quality, result.fastMode);
      return {
        bytesWritten: written.bytesWritten,
        quality: result.quality,
        budgetMet: written.bytesWritten <= result.targetBytes,
        targetBytes: result.targetBytes,
        metrics: written.metrics,
      };
    }
    // Legacy path: toFile returns bytesWritten directly, no metrics
    const bytesWritten = await this.toFile(path, result.format, result.quality, result.fastMode);
    return {
      bytesWritten,
      quality: result.quality,
      budgetMet: bytesWritten <= result.targetBytes,
      targetBytes: result.targetBytes,
      metrics: result.metrics,
    };
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
  normalizeTargetBytesOptions,
  encodeWithinByteBudget,
  resolveEncodeProfile,
  createGetErrorCategory,
  attachPrototypeMethods,
};
