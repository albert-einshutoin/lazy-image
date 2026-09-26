import { createHash } from 'node:crypto';
import { createRequire } from 'node:module';
import fs from 'node:fs/promises';
import sharp from 'sharp';

const require = createRequire(import.meta.url);
const { ssim } = require('ssim.js');
export const sha256 = (bytes) => createHash('sha256').update(bytes).digest('hex');
export const ssimOptions = Object.freeze({
  windowSize: 8, k1: 0.01, k2: 0.03, bitDepth: 8,
  downsample: 'original', ssim: 'weber', maxSize: 256, rgb2grayVersion: 'integer',
});

export async function readChecked(file, expectedHash) {
  const bytes = await fs.readFile(file);
  const actual = sha256(bytes);
  if (actual !== expectedHash) throw new Error(`SHA-256 mismatch: ${file}: ${actual} != ${expectedHash}`);
  return bytes;
}

export function rgbPsnr(a, b) {
  if (a.width !== b.width || a.height !== b.height) throw new Error('PSNR: dimensions differ');
  if (a.data.length !== b.data.length || a.data.length !== a.width * a.height * 4) {
    throw new Error('PSNR: invalid RGBA pixel length');
  }
  let sum = 0;
  for (let i = 0; i < a.data.length; i += 4) {
    for (let channel = 0; channel < 3; channel++) {
      const diff = a.data[i + channel] - b.data[i + channel];
      sum += diff * diff;
    }
  }
  if (sum === 0) return { status: 'perfect', value: 'Infinity' };
  const mse = sum / (a.width * a.height * 3);
  return { status: 'measured', value: 10 * Math.log10((255 * 255) / mse) };
}

async function decoded(bytes) {
  const metadata = await sharp(bytes).metadata();
  const { data, info } = await sharp(bytes).toColourspace('srgb').ensureAlpha()
    .raw({ depth: 'uchar' }).toBuffer({ resolveWithObject: true });
  let transparentPixels = 0;
  for (let i = 3; i < data.length; i += 4) if (data[i] !== 255) transparentPixels++;
  return { data, width: info.width, height: info.height, transparentPixels,
    metadata: { width: metadata.width, height: metadata.height, format: metadata.format,
      orientation: metadata.orientation ?? null, hasIcc: Boolean(metadata.icc),
      hasAlphaChannel: Boolean(metadata.hasAlpha), transparentPixels } };
}

export async function pixelMetadata(bytes) {
  const image = await decoded(bytes);
  return image.metadata;
}

export async function makeReference(inputBytes, { width, height }) {
  const input = await pixelMetadata(inputBytes);
  // Materialize oriented sRGB first so ICC conversion precedes resizing in the reference.
  const orientedSrgb = await sharp(inputBytes).autoOrient().toColourspace('srgb')
    .png({ compressionLevel: 9 }).toBuffer();
  const bytes = await sharp(orientedSrgb)
    .resize({ width, height, fit: 'inside', kernel: 'lanczos3',
      withoutEnlargement: true, fastShrinkOnLoad: false })
    .png({ compressionLevel: 9 }).toBuffer();
  const reference = await pixelMetadata(bytes);
  return { bytes, input, reference, orientedSrgbSha256: sha256(orientedSrgb) };
}

export async function score(referenceBytes, outputBytes) {
  let reference;
  let output;
  try {
    [reference, output] = await Promise.all([decoded(referenceBytes), decoded(outputBytes)]);
  } catch (error) {
    return { status: 'error', reason: `decode failed: ${error.message}`,
      ssim: { status: 'error', value: null, reason: error.message },
      psnr: { status: 'error', value: null, reason: error.message } };
  }
  const reason = reference.width !== output.width || reference.height !== output.height
    ? `dimensions differ: reference ${reference.width}x${reference.height}, output ${output.width}x${output.height}`
    : reference.transparentPixels || output.transparentPixels
      ? `transparent pixels: reference ${reference.transparentPixels}, output ${output.transparentPixels}` : null;
  if (reason) return { status: 'unmeasured', reason, outputMetadata: output.metadata,
    ssim: { status: 'unmeasured', value: null, reason },
    psnr: { status: 'unmeasured', value: null, reason } };
  try {
    const psnr = rgbPsnr(reference, output);
    const value = ssim(
      { data: new Uint8ClampedArray(reference.data), width: reference.width, height: reference.height },
      { data: new Uint8ClampedArray(output.data), width: output.width, height: output.height },
      ssimOptions,
    ).mssim;
    // A negative SSIM is valid for sufficiently dissimilar signals; only nonfinite or >1 is suspect.
    if (!Number.isFinite(value) || value > 1 + 1e-10 || value < -1 - 1e-10) {
      throw new Error(`SSIM outside numerical range: ${value}`);
    }
    return { status: 'measured', outputMetadata: output.metadata,
      ssim: { status: 'measured', value }, psnr,
      ssimDownsampleFactor: Math.max(1, Math.round(Math.min(reference.width, reference.height) / ssimOptions.maxSize)) };
  } catch (error) {
    return { status: 'error', reason: error.message, outputMetadata: output.metadata,
      ssim: { status: 'error', value: null, reason: error.message },
      psnr: { status: 'error', value: null, reason: error.message } };
  }
}
