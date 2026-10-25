export const VERSION = '2.0.0';

export const ERROR_CATEGORIES = Object.freeze({
  UserError: 'UserError',
  CodecError: 'CodecError',
  ResourceLimit: 'ResourceLimit',
  InternalBug: 'InternalBug',
});

// Keep this table explicit instead of deriving categories from number ranges.
// Native codes within the same range intentionally have different recovery
// semantics, and upload preflight must make the same decision as server-side
// processing when callers branch on `code` and `category`.
const ERROR_CODE_CATEGORIES = Object.freeze({
  E111: ERROR_CATEGORIES.CodecError,
  E122: ERROR_CATEGORIES.ResourceLimit,
  E123: ERROR_CATEGORIES.ResourceLimit,
  E131: ERROR_CATEGORIES.CodecError,
  E203: ERROR_CATEGORIES.UserError,
  E204: ERROR_CATEGORIES.UserError,
  E300: ERROR_CATEGORIES.CodecError,
  E400: ERROR_CATEGORIES.UserError,
  E401: ERROR_CATEGORIES.UserError,
  E500: ERROR_CATEGORIES.UserError,
  E501: ERROR_CATEGORIES.ResourceLimit,
  E502: ERROR_CATEGORIES.ResourceLimit,
  E503: ERROR_CATEGORIES.CodecError,
  E901: ERROR_CATEGORIES.InternalBug,
});

const FORMATS = new Set(['jpeg', 'webp']);
const OUTPUT_KINDS = new Set(['blob', 'arrayBuffer', 'uint8Array']);
const FITS = new Set(['inside', 'cover', 'fill']);
const FLOOR_POLICIES = new Set(['best-effort', 'strict']);
const PROFILES = new Set(['upload-safe', 'avatar', 'attachment', 'balanced']);

const PROFILE_DEFAULTS = {
  'upload-safe': {
    minQuality: 45,
    maxQuality: 85,
  },
  avatar: {
    maxWidth: 512,
    maxHeight: 512,
    minQuality: 45,
    maxQuality: 82,
  },
  attachment: {
    minQuality: 40,
    maxQuality: 86,
  },
  balanced: {
    minQuality: 50,
    maxQuality: 80,
  },
};

export class LazyImageWasmError extends Error {
  constructor(code, message, options = {}) {
    super(message);
    this.name = 'LazyImageWasmError';
    this.code = code;
    this.category = options.category ?? categoryForCode(code);
    this.recoverable =
      options.recoverable ??
      (this.category === ERROR_CATEGORIES.UserError ||
        this.category === ERROR_CATEGORIES.ResourceLimit);
    if (options.recoveryHint) {
      this.recoveryHint = options.recoveryHint;
    }
  }
}

export function categoryForCode(code) {
  return ERROR_CODE_CATEGORIES[code] ?? ERROR_CATEGORIES.InternalBug;
}

export function throwWasmError(code, message, options) {
  throw new LazyImageWasmError(code, message, options);
}

export const DEFAULT_LIMITS = Object.freeze({ maxBytes: 32 * 1024 * 1024, maxPixels: 40_000_000, timeoutMs: 5000 });

export async function inputToBytes(input, maxBytes = DEFAULT_LIMITS.maxBytes) {
  if (input instanceof ArrayBuffer) {
    return new Uint8Array(input);
  }
  if (ArrayBuffer.isView(input)) {
    return new Uint8Array(input.buffer, input.byteOffset, input.byteLength);
  }
  if (input && typeof input.arrayBuffer === 'function') {
    if (!Number.isSafeInteger(input.size) || input.size < 0) {
      throwWasmError('E400', 'Blob/File input requires a finite byte size. Pass bounded bytes for other inputs.');
    }
    if (input.size > maxBytes) throwWasmError('E123', `Input exceeds limits.maxBytes (${maxBytes}).`);
    const buffer = await input.arrayBuffer();
    return new Uint8Array(buffer);
  }
  throwWasmError('E400', 'Unsupported input type for Wasm upload optimization.', {
    recoveryHint: 'Pass a File, Blob, ArrayBuffer, or Uint8Array.',
  });
}

export function detectInputFormat(bytes) {
  if (bytes.length >= 3 && bytes[0] === 0xff && bytes[1] === 0xd8 && bytes[2] === 0xff) {
    return 'jpeg';
  }
  if (
    bytes.length >= 8 &&
    bytes[0] === 0x89 &&
    bytes[1] === 0x50 &&
    bytes[2] === 0x4e &&
    bytes[3] === 0x47 &&
    bytes[4] === 0x0d &&
    bytes[5] === 0x0a &&
    bytes[6] === 0x1a &&
    bytes[7] === 0x0a
  ) {
    return 'png';
  }
  if (
    bytes.length >= 12 &&
    bytes[0] === 0x52 &&
    bytes[1] === 0x49 &&
    bytes[2] === 0x46 &&
    bytes[3] === 0x46 &&
    bytes[8] === 0x57 &&
    bytes[9] === 0x45 &&
    bytes[10] === 0x42 &&
    bytes[11] === 0x50
  ) {
    return 'webp';
  }
  throwWasmError('E111', 'Unsupported input image format for the Wasm MVP.', {
    recoveryHint: 'Use JPEG, PNG, or WebP input for the first upload-preflight slice.',
  });
}

export function normalizeOptions(options = {}, runtimeDefaults = {}) {
  const profile = options.profile ?? 'upload-safe';
  if (!PROFILES.has(profile)) {
    throwWasmError('E401', `Invalid Wasm profile: ${profile}`);
  }

  const defaults = PROFILE_DEFAULTS[profile];
  const normalized = {
    fit: 'inside',
    output: runtimeDefaults.output ?? 'blob',
    stripMetadata: true,
    qualityFloorPolicy: 'best-effort',
    profile,
    ...defaults,
    ...options,
  };

  if (!FORMATS.has(normalized.format)) {
    throwWasmError('E400', `Unsupported Wasm output format: ${normalized.format}`, {
      recoveryHint: 'Use "jpeg" or "webp". AVIF is deferred for the MVP.',
    });
  }
  if (!OUTPUT_KINDS.has(normalized.output)) {
    throwWasmError('E400', `Unsupported Wasm output kind: ${normalized.output}`);
  }
  if (!FITS.has(normalized.fit)) {
    throwWasmError('E204', `Unsupported resize fit: ${normalized.fit}`);
  }
  if (!FLOOR_POLICIES.has(normalized.qualityFloorPolicy)) {
    throwWasmError('E400', `Unsupported qualityFloorPolicy: ${normalized.qualityFloorPolicy}`);
  }

  normalized.minQuality = normalizeQuality(normalized.minQuality, 'minQuality');
  normalized.maxQuality = normalizeQuality(normalized.maxQuality, 'maxQuality');
  if (normalized.minQuality > normalized.maxQuality) {
    throwWasmError('E400', 'minQuality must be less than or equal to maxQuality.');
  }

  for (const key of ['maxWidth', 'maxHeight', 'targetBytes']) {
    if (normalized[key] !== undefined) {
      normalized[key] = normalizePositiveInteger(normalized[key], key);
    }
  }

  const limits = { ...normalized.limits };
  for (const key of ['maxBytes', 'maxPixels', 'timeoutMs']) {
    if (limits[key] === undefined) limits[key] = DEFAULT_LIMITS[key];
    limits[key] = normalizePositiveInteger(limits[key], `limits.${key}`);
  }
  normalized.limits = limits;

  return normalized;
}

export function validateInputLimits(bytes, options) {
  if (options.limits.maxBytes !== undefined && bytes.byteLength > options.limits.maxBytes) {
    throwWasmError('E123', `Input exceeds limits.maxBytes (${options.limits.maxBytes}).`);
  }
}

export function validatePixelLimits(width, height, options) {
  if (!Number.isSafeInteger(width) || !Number.isSafeInteger(height) || width <= 0 || height <= 0 || width > 32768 || height > 32768) {
    throwWasmError('E122', 'Image dimensions must be positive integers at most 32768.');
  }
  const pixels = width * height;
  if (options.limits.maxPixels !== undefined && pixels > options.limits.maxPixels) {
    throwWasmError('E122', `Input exceeds limits.maxPixels (${options.limits.maxPixels}).`);
  }
}

// Read container dimensions before handing bytes to an allocating codec. This
// bounds admission; it is not a cap on a codec's working memory or CPU time.
export function preflightDimensions(bytes, format, options) {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const invalid = () => throwWasmError('E111', 'Malformed or unsupported image dimension header.');
  let width;
  let height;
  if (format === 'png') {
    if (bytes.length < 33 || view.getUint32(8) !== 13 || view.getUint32(12) !== 0x49484452) invalid();
    width = view.getUint32(16);
    height = view.getUint32(20);
  } else if (format === 'jpeg') {
    let offset = 2;
    while (offset < bytes.length) {
      if (bytes[offset++] !== 0xff) invalid();
      while (offset < bytes.length && bytes[offset] === 0xff) offset++;
      if (offset >= bytes.length) invalid();
      const marker = bytes[offset++];
      if (marker === 0xda || marker === 0xd9 || marker === 0x00) invalid();
      if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
      if (offset + 2 > bytes.length) invalid();
      const length = view.getUint16(offset);
      if (length < 2 || offset + length > bytes.length) invalid();
      if ((marker >= 0xc0 && marker <= 0xcf) && ![0xc4, 0xc8, 0xcc].includes(marker)) {
        if (length < 8) invalid();
        height = view.getUint16(offset + 3);
        width = view.getUint16(offset + 5);
        break;
      }
      offset += length;
    }
  } else if (format === 'webp') {
    if (bytes.length < 20 || view.getUint32(4, true) + 8 !== bytes.length) invalid();
    let canvas;
    for (let offset = 12; offset < bytes.length;) {
      if (offset + 8 > bytes.length) invalid();
      const tag = view.getUint32(offset);
      const length = view.getUint32(offset + 4, true);
      const start = offset + 8;
      const end = start + length;
      if (end > bytes.length) invalid();
      if (tag === 0x414e494d || tag === 0x414e4d46) invalid(); // ANIM / ANMF
      if (tag === 0x56503858) { // VP8X
        if (length !== 10 || canvas || (bytes[start] & 2)) invalid();
        const u24 = i => bytes[i] + bytes[i + 1] * 256 + bytes[i + 2] * 65536;
        canvas = [u24(start + 4) + 1, u24(start + 7) + 1];
        validatePixelLimits(...canvas, options);
      } else if (tag === 0x56503820 || tag === 0x5650384c) { // VP8 / VP8L
        if (width !== undefined) invalid();
        if (tag === 0x56503820) {
          if (length < 10 || bytes[start + 3] !== 0x9d || bytes[start + 4] !== 1 || bytes[start + 5] !== 0x2a) invalid();
          width = view.getUint16(start + 6, true) & 0x3fff;
          height = view.getUint16(start + 8, true) & 0x3fff;
        } else {
          if (length < 5 || bytes[start] !== 0x2f) invalid();
          const bits = view.getUint32(start + 1, true);
          width = (bits & 0x3fff) + 1;
          height = ((bits >>> 14) & 0x3fff) + 1;
        }
      }
      offset = end + (length & 1);
      if (offset > bytes.length) invalid();
    }
    if (canvas && (canvas[0] !== width || canvas[1] !== height)) invalid();
  } else invalid();
  if (width === undefined || height === undefined) invalid();
  validatePixelLimits(width, height, options);
  return { width, height };
}

export function resolveTargetDimensions(width, height, options) {
  const maxWidth = options.maxWidth;
  const maxHeight = options.maxHeight;
  if (!maxWidth && !maxHeight) {
    return { width, height };
  }

  if (options.fit === 'cover') {
    if (!maxWidth || !maxHeight) {
      throwWasmError('E203', 'cover fit requires both maxWidth and maxHeight.');
    }
    return {
      width: maxWidth,
      height: maxHeight,
    };
  }

  if (options.fit === 'fill') {
    return {
      width: maxWidth ?? width,
      height: maxHeight ?? height,
    };
  }

  const widthRatio = maxWidth ? maxWidth / width : 1;
  const heightRatio = maxHeight ? maxHeight / height : 1;
  const ratio = Math.min(widthRatio, heightRatio, 1);
  return {
    width: Math.max(1, Math.round(width * ratio)),
    height: Math.max(1, Math.round(height * ratio)),
  };
}

export function checkAbort(signal) {
  if (signal?.aborted) {
    throwWasmError('E500', 'Wasm upload optimization was aborted.', {
      recoverable: true,
    });
  }
}

export function checkTimeout(startMs, stage, options, now) {
  const timeoutMs = options.limits?.timeoutMs;
  if (timeoutMs === undefined) {
    return;
  }

  const elapsedMs = nowMs(now) - startMs;
  if (elapsedMs > timeoutMs) {
    throwWasmError('E123', `Wasm upload optimization exceeded limits.timeoutMs (${timeoutMs}) at ${stage}.`, {
      recoveryHint:
        'Increase limits.timeoutMs, reduce max dimensions, or use AbortController for external cancellation.',
    });
  }
}

export function exactArrayBuffer(buffer) {
  if (buffer instanceof ArrayBuffer) {
    return buffer;
  }
  if (ArrayBuffer.isView(buffer)) {
    return buffer.buffer.slice(buffer.byteOffset, buffer.byteOffset + buffer.byteLength);
  }
  throwWasmError('E901', 'Codec returned an unsupported buffer type.', {
    recoverable: false,
  });
}

export function makeResultData(arrayBuffer, output, format) {
  const exact = exactArrayBuffer(arrayBuffer);
  if (output === 'arrayBuffer') {
    return exact;
  }
  if (output === 'uint8Array') {
    return new Uint8Array(exact);
  }
  if (typeof Blob !== 'function') {
    throwWasmError('E501', 'Blob output is not available in this runtime.', {
      recoveryHint: 'Use output: "arrayBuffer" or output: "uint8Array".',
    });
  }
  return new Blob([exact], { type: format === 'jpeg' ? 'image/jpeg' : 'image/webp' });
}

export function nowMs(now) {
  return typeof now === 'function' ? now() : performance.now();
}

function normalizeQuality(value, key) {
  const quality = value === undefined ? undefined : Number(value);
  if (!Number.isInteger(quality) || quality < 1 || quality > 100) {
    throwWasmError('E400', `${key} must be an integer from 1 to 100.`);
  }
  return quality;
}

function normalizePositiveInteger(value, key) {
  const normalized = Number(value);
  if (!Number.isSafeInteger(normalized) || normalized <= 0) {
    throwWasmError('E400', `${key} must be a positive integer.`);
  }
  return normalized;
}
