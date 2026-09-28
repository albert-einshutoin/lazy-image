import jpegDecode, { init as initJpegDecode } from '@jsquash/jpeg/decode.js';
import jpegEncode, { init as initJpegEncode } from '@jsquash/jpeg/encode.js';
import pngDecode, { init as initPngDecode } from '@jsquash/png/decode.js';
import resizeImage, { initResize } from '@jsquash/resize';
import webpDecode, { init as initWebpDecode } from '@jsquash/webp/decode.js';
import webpEncode, { init as initWebpEncode } from '@jsquash/webp/encode.js';

import {
  LazyImageWasmError,
  VERSION,
  checkAbort,
  checkTimeout,
  detectInputFormat,
  exactArrayBuffer,
  inputToBytes,
  makeResultData,
  normalizeOptions,
  nowMs,
  resolveTargetDimensions,
  throwWasmError,
  validateInputLimits,
  validatePixelLimits,
} from './shared.js';

const DEFAULT_RUNTIME = 'browser';
const DECODE_WASM = {
  jpeg: 'mozjpeg_dec.wasm',
  png: 'squoosh_png_bg.wasm',
  webp: 'webp_dec.wasm',
};

function codecFailureMessage(error) {
  try {
    return typeof error?.message === 'string' ? error.message : 'cause unavailable';
  } catch {
    return 'cause unavailable';
  }
}

function wasmDeliveryHint(asset) {
  return `For the bundled Worker, check that ${asset} is served beside worker.js ` +
    '(dist/ is the HTTP root in the README). In DevTools Network, inspect the .wasm request URL ' +
    'and HTTP status; confirm the response contains Wasm bytes, not a 404 page or HTML.';
}

export async function createUploadOptimizer(options = {}) {
  const runtime = options.runtime ?? DEFAULT_RUNTIME;
  const defaultOutput = options.defaultOutput ?? 'blob';
  const now = options.now ?? (() => performance.now());
  const instantiateStart = nowMs(now);
  await initializeCodecs(options);
  const instantiateMs = nowMs(now) - instantiateStart;

  return {
    async optimizeUpload(input, optimizeOptions) {
      return optimizeUploadWithRuntime(input, optimizeOptions, {
        defaultOutput,
        instantiateMs,
        now,
        runtime,
      });
    },
  };
}

export async function optimizeUpload(input, options) {
  const optimizer = await createUploadOptimizer();
  return optimizer.optimizeUpload(input, options);
}

async function optimizeUploadWithRuntime(input, options, runtimeOptions) {
  const totalStart = nowMs(runtimeOptions.now);
  const normalized = normalizeOptions(options, { output: runtimeOptions.defaultOutput });

  checkAbort(normalized.signal);
  const bytes = await inputToBytes(input);
  checkTimeout(totalStart, 'input-read', normalized, runtimeOptions.now);
  validateInputLimits(bytes, normalized);

  const inputFormat = detectInputFormat(bytes);
  const decodeStart = nowMs(runtimeOptions.now);
  const imageData = await decodeImage(bytes, inputFormat);
  const decodeMs = nowMs(runtimeOptions.now) - decodeStart;
  checkTimeout(totalStart, 'decode', normalized, runtimeOptions.now);
  validatePixelLimits(imageData.width, imageData.height, normalized);

  checkAbort(normalized.signal);
  const opsStart = nowMs(runtimeOptions.now);
  const targetDimensions = resolveTargetDimensions(imageData.width, imageData.height, normalized);
  const processed =
    targetDimensions.width === imageData.width && targetDimensions.height === imageData.height
      ? imageData
      : await resizeWithCodec(imageData, {
          width: targetDimensions.width,
          height: targetDimensions.height,
          fitMethod: normalized.fit === 'cover' ? 'contain' : 'stretch',
          method: 'lanczos3',
          premultiply: true,
          linearRGB: true,
        });
  const opsMs = nowMs(runtimeOptions.now) - opsStart;
  checkTimeout(totalStart, 'process', normalized, runtimeOptions.now);

  checkAbort(normalized.signal);
  const encodeStart = nowMs(runtimeOptions.now);
  const encoded = await encodeWithPolicy(processed, normalized, runtimeOptions.now, totalStart);
  const encodeMs = nowMs(runtimeOptions.now) - encodeStart;
  const totalMs = nowMs(runtimeOptions.now) - totalStart;
  checkTimeout(totalStart, 'encode', normalized, runtimeOptions.now);
  const resultBuffer = exactArrayBuffer(encoded.data);

  const metrics = {
    version: VERSION,
    runtime: runtimeOptions.runtime,
    bytesIn: bytes.byteLength,
    bytesOut: resultBuffer.byteLength,
    formatIn: inputFormat,
    formatOut: normalized.format,
    widthIn: imageData.width,
    heightIn: imageData.height,
    widthOut: processed.width,
    heightOut: processed.height,
    qualityUsed: encoded.qualityUsed,
    budgetMet: encoded.budgetMet,
    targetBytes: normalized.targetBytes,
    instantiateMs: runtimeOptions.instantiateMs,
    decodeMs,
    opsMs,
    encodeMs,
    totalMs,
    firstEncodeMs: encoded.firstEncodeMs,
    metadataStripped: normalized.stripMetadata,
    policyViolations: encoded.budgetMet ? [] : ['targetBytes'],
    memory: {
      peakBytes: undefined,
      source: 'unavailable',
    },
  };

  return {
    data: makeResultData(resultBuffer, normalized.output, normalized.format),
    format: normalized.format,
    qualityUsed: encoded.qualityUsed,
    bytesOut: resultBuffer.byteLength,
    budgetMet: encoded.budgetMet,
    targetBytes: normalized.targetBytes,
    metrics,
  };
}

async function initializeCodecs(options) {
  if (!options.wasmModules || Object.keys(options.wasmModules).length === 0) {
    return undefined;
  }
  const modules = options.wasmModules ?? {};
  const tasks = [];
  const codecs = [
    ['jpegDecode', initJpegDecode, 'E131', 'jpeg decoder', DECODE_WASM.jpeg],
    ['jpegEncode', initJpegEncode, 'E300', 'jpeg encoder', 'mozjpeg_enc.wasm'],
    ['pngDecode', initPngDecode, 'E131', 'png decoder', DECODE_WASM.png],
    ['resize', initResize, 'E503', 'resize', 'squoosh_resize_bg.wasm'],
    ['webpDecode', initWebpDecode, 'E131', 'webp decoder', DECODE_WASM.webp],
    ['webpEncode', initWebpEncode, 'E300', 'webp encoder', 'webp_enc_simd.wasm or webp_enc.wasm (the selected variant)'],
  ];
  for (const [key, init, code, stage, asset] of codecs) {
    if (modules[key]) tasks.push(initializeCodec(modules[key], init, code, stage, asset));
  }

  await Promise.all(tasks);
}

async function initializeCodec(module, init, code, stage, asset) {
  try {
    await init(await toWasmModule(module));
  } catch (error) {
    if (error instanceof LazyImageWasmError) throw error;
    throwWasmError(code, `${stage} codec initialization failed: ${codecFailureMessage(error)}`, {
      recoveryHint: `${wasmDeliveryHint(asset)} If wasmModules was supplied, check that its module matches the codec.`,
    });
  }
}

async function toWasmModule(input) {
  if (input instanceof WebAssembly.Module) {
    return input;
  }
  if (input instanceof ArrayBuffer) {
    return WebAssembly.compile(input);
  }
  if (ArrayBuffer.isView(input)) {
    return WebAssembly.compile(input.buffer.slice(input.byteOffset, input.byteOffset + input.byteLength));
  }
  return input;
}

async function decodeImage(bytes, inputFormat) {
  const buffer = exactArrayBuffer(bytes);
  try {
    if (inputFormat === 'jpeg') return await jpegDecode(buffer, { preserveOrientation: true });
    if (inputFormat === 'png') return await pngDecode(buffer);
    if (inputFormat === 'webp') return await webpDecode(buffer);
  } catch (error) {
    if (error instanceof LazyImageWasmError) throw error;
    throwWasmError('E131', `${inputFormat} decoder failed during codec initialization or image decoding: ${codecFailureMessage(error)}`, {
      recoveryHint: `${wasmDeliveryHint(DECODE_WASM[inputFormat])} If delivery is valid, check the input image for corruption and the codec for a processing failure.`,
    });
  }
  throwWasmError('E111', `Unsupported input format: ${inputFormat}`);
}

async function resizeWithCodec(imageData, options) {
  try {
    return await resizeImage(imageData, options);
  } catch (error) {
    if (error instanceof LazyImageWasmError) throw error;
    throwWasmError('E503', `resize codec failed during initialization or image resizing: ${codecFailureMessage(error)}`, {
      recoveryHint: `${wasmDeliveryHint('squoosh_resize_bg.wasm')} If delivery is valid, check the codec processing failure.`,
    });
  }
}

async function encodeWithPolicy(imageData, options, now, totalStart) {
  if (!options.targetBytes) {
    const start = nowMs(now);
    const data = await encodeAtQuality(imageData, options.format, options.maxQuality);
    const firstEncodeMs = nowMs(now) - start;
    checkTimeout(totalStart, 'encode', options, now);
    return {
      data,
      qualityUsed: options.maxQuality,
      budgetMet: true,
      firstEncodeMs,
    };
  }

  let low = options.minQuality;
  let high = options.maxQuality;
  let firstEncodeMs;
  let bestUnderBudget = null;
  let smallest = null;

  while (low <= high) {
    checkAbort(options.signal);
    checkTimeout(totalStart, 'encode', options, now);
    const quality = Math.floor((low + high) / 2);
    const start = nowMs(now);
    const data = await encodeAtQuality(imageData, options.format, quality);
    const elapsed = nowMs(now) - start;
    checkTimeout(totalStart, 'encode', options, now);
    if (firstEncodeMs === undefined) {
      firstEncodeMs = elapsed;
    }

    const candidate = {
      data,
      qualityUsed: quality,
      bytesOut: exactArrayBuffer(data).byteLength,
    };

    if (!smallest || candidate.bytesOut < smallest.bytesOut) {
      smallest = candidate;
    }
    if (candidate.bytesOut <= options.targetBytes) {
      bestUnderBudget = candidate;
      low = quality + 1;
    } else {
      high = quality - 1;
    }
  }

  const selected = bestUnderBudget ?? smallest;
  const budgetMet = selected.bytesOut <= options.targetBytes;
  if (!budgetMet && options.qualityFloorPolicy === 'strict') {
    throwWasmError('E502', `Unable to meet targetBytes=${options.targetBytes} within quality floor.`, {
      recoveryHint: 'Increase targetBytes, reduce max dimensions, or use qualityFloorPolicy: "best-effort".',
    });
  }

  return {
    data: selected.data,
    qualityUsed: selected.qualityUsed,
    budgetMet,
    firstEncodeMs,
  };
}

async function encodeAtQuality(imageData, format, quality) {
  try {
    if (format === 'jpeg') {
      return await jpegEncode(imageData, {
        quality,
        progressive: true,
        optimize_coding: true,
        auto_subsample: true,
      });
    }
    if (format === 'webp') {
      return await webpEncode(imageData, {
        quality,
        lossless: 0,
      });
    }
  } catch (error) {
    if (error instanceof LazyImageWasmError) throw error;
    const asset = format === 'webp' ? 'webp_enc_simd.wasm or webp_enc.wasm (the selected variant)' :
      'mozjpeg_enc.wasm';
    throwWasmError('E300', `${format} encoder failed during codec initialization or image encoding: ${codecFailureMessage(error)}`, {
      recoveryHint: `${wasmDeliveryHint(asset)} If delivery is valid, check the codec and encoding options.`,
    });
  }
  throwWasmError('E400', `Unsupported output format: ${format}`);
}
