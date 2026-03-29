const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const indexJsPath = path.join(root, 'index.js');
const indexDtsPath = path.join(root, 'index.d.ts');

const jsHelperBlock = `

// High-level helpers and streaming API
const { createStreamingPipeline } = require('./streaming/pipeline')

function getErrorCategory(err) {
  if (!err) return null

  const { ErrorCategory, ErrorCode } = nativeBinding

  if (typeof err.errorCategory === 'number') return err.errorCategory
  if (typeof err.category === 'number') return err.category

  if (typeof err.code === 'string') {
    switch (err.code) {
      case 'LAZY_IMAGE_USER_ERROR':
        return ErrorCategory.UserError
      case 'LAZY_IMAGE_CODEC_ERROR':
        return ErrorCategory.CodecError
      case 'LAZY_IMAGE_RESOURCE_LIMIT':
        return ErrorCategory.ResourceLimit
      case 'LAZY_IMAGE_INTERNAL_BUG':
        return ErrorCategory.InternalBug
      default:
        break
    }
  }

  let numericCode = null
  if (typeof err.errorCode === 'string' && /^E[0-9]{3}$/.test(err.errorCode)) {
    numericCode = parseInt(err.errorCode.slice(1), 10)
  } else if (typeof err.errorCode === 'number') {
    numericCode = err.errorCode
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
        return ErrorCategory.UserError
      case ErrorCode.UnsupportedFormat:
      case ErrorCode.DecodeFailed:
      case ErrorCode.CorruptedImage:
      case ErrorCode.EncodeFailed:
      case ErrorCode.UnsupportedColorSpace:
      case ErrorCode.ResizeFailed:
        return ErrorCategory.CodecError
      case ErrorCode.DimensionExceedsLimit:
      case ErrorCode.PixelCountExceedsLimit:
      case ErrorCode.FileReadFailed:
      case ErrorCode.MmapFailed:
      case ErrorCode.FileWriteFailed:
      case ErrorCode.FirewallViolation:
        return ErrorCategory.ResourceLimit
      case ErrorCode.InternalPanic:
      case ErrorCode.Generic:
        return ErrorCategory.InternalBug
      default:
        break
    }

    const bucket = Math.floor(numericCode / 100)
    if (bucket === 1 || bucket === 2 || bucket === 4) return ErrorCategory.UserError
    if (bucket === 3) return ErrorCategory.ResourceLimit
    if (bucket === 9) return ErrorCategory.InternalBug
  }

  return null
}

module.exports.getErrorCategory = getErrorCategory
module.exports.createStreamingPipeline = createStreamingPipeline

const DEFAULT_QUALITY_BY_FORMAT = {
  jpeg: 85,
  jpg: 85,
  webp: 80,
  avif: 60,
}

const TARGET_BYTES_SUPPORTED_FORMATS = new Set(['jpeg', 'jpg', 'webp', 'avif'])
const DEFAULT_TARGET_BYTES_MIN_QUALITY = 30

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
}

function clampQuality(v) {
  const n = Number(v)
  if (!Number.isFinite(n)) return undefined
  return Math.max(0, Math.min(100, Math.round(n)))
}

function normalizeTargetBytesOptions(format, options = {}) {
  const normalizedFormat = String(format || '').toLowerCase()
  if (!TARGET_BYTES_SUPPORTED_FORMATS.has(normalizedFormat)) {
    throw new Error(\`Target-bytes mode supports jpeg/webp/avif only: \${format}\`)
  }

  const rawBudget = options.targetBytes ?? options.maxBytes
  const targetBytes = Number(rawBudget)
  if (!Number.isFinite(targetBytes) || targetBytes <= 0) {
    throw new Error('targetBytes must be a positive number')
  }

  const minQuality = clampQuality(options.minQuality ?? DEFAULT_TARGET_BYTES_MIN_QUALITY)
  const maxQuality = clampQuality(options.maxQuality ?? 100)
  if (minQuality == null || maxQuality == null || minQuality <= 0 || maxQuality <= 0) {
    throw new Error('minQuality/maxQuality must be integers between 1 and 100')
  }
  if (minQuality > maxQuality) {
    throw new Error('minQuality must be less than or equal to maxQuality')
  }

  const policy = options.qualityFloorPolicy == null ? 'best-effort' : String(options.qualityFloorPolicy)
  if (!['best-effort', 'strict'].includes(policy)) {
    throw new Error(\`Unsupported qualityFloorPolicy: \${options.qualityFloorPolicy}\`)
  }

  return {
    format: normalizedFormat === 'jpg' ? 'jpeg' : normalizedFormat,
    targetBytes: Math.round(targetBytes),
    minQuality,
    maxQuality,
    fastMode: Boolean(options.fastMode),
    qualityFloorPolicy: policy,
  }
}

async function encodeWithinByteBudget(engine, format, options) {
  const config = normalizeTargetBytesOptions(format, options)
  let low = config.minQuality
  let high = config.maxQuality
  let bestUnder = null
  let bestOver = null

  while (low <= high) {
    const quality = Math.floor((low + high) / 2)
    const result = await engine.toBufferWithMetrics(config.format, quality, config.fastMode)
    const candidate = {
      data: result.data,
      metrics: result.metrics,
      quality,
      bytesOut: result.metrics?.bytesOut ?? result.data.length,
    }

    if (candidate.bytesOut <= config.targetBytes) {
      bestUnder = candidate
      low = quality + 1
    } else {
      if (
        !bestOver ||
        candidate.bytesOut < bestOver.bytesOut ||
        (candidate.bytesOut === bestOver.bytesOut && quality > bestOver.quality)
      ) {
        bestOver = candidate
      }
      high = quality - 1
    }
  }

  const chosen = bestUnder || bestOver
  if (!chosen) {
    throw new Error('Failed to encode any candidate while searching for target bytes')
  }

  const budgetMet = chosen.bytesOut <= config.targetBytes
  if (!budgetMet && config.qualityFloorPolicy === 'strict') {
    throw new Error(\`Unable to meet targetBytes=\${config.targetBytes} within quality range \${config.minQuality}-\${config.maxQuality}\`)
  }

  return {
    ...chosen,
    budgetMet,
    targetBytes: config.targetBytes,
    format: config.format,
    fastMode: config.fastMode,
    qualityFloorPolicy: config.qualityFloorPolicy,
  }
}

function resolveEncodeProfile(format, profile = 'balanced', quality) {
  const normalizedFormat = String(format || '').toLowerCase()
  if (!['jpeg', 'jpg', 'png', 'webp', 'avif'].includes(normalizedFormat)) {
    throw new Error(\`Unsupported format for profile: \${format}\`)
  }

  const normalizedProfile = String(profile || 'balanced').toLowerCase()
  if (!Object.prototype.hasOwnProperty.call(ENCODE_PROFILE_CONFIG, normalizedProfile)) {
    throw new Error(\`Unsupported encode profile: \${profile}\`)
  }

  const profileConfig = ENCODE_PROFILE_CONFIG[normalizedProfile]
  const formatKey = normalizedFormat === 'jpg' ? 'jpeg' : normalizedFormat
  const baseQuality = clampQuality(quality) ?? DEFAULT_QUALITY_BY_FORMAT[formatKey]
  const delta = (profileConfig[formatKey] || profileConfig.png).qualityDelta
  const resolvedQuality = baseQuality == null ? undefined : clampQuality(baseQuality + delta)
  const fastMode = Boolean((profileConfig[formatKey] || profileConfig.png).fastMode)

  return {
    format: normalizedFormat,
    quality: resolvedQuality,
    fastMode,
  }
}

module.exports.resolveEncodeProfile = resolveEncodeProfile

if (nativeBinding && nativeBinding.ImageEngine && nativeBinding.ImageEngine.prototype) {
  const p = nativeBinding.ImageEngine.prototype

  p.toBufferProfile = function toBufferProfile(format, profile = 'balanced', quality) {
    const resolved = resolveEncodeProfile(format, profile, quality)
    return this.toBuffer(resolved.format, resolved.quality, resolved.fastMode)
  }

  p.toBufferWithMetricsProfile = function toBufferWithMetricsProfile(format, profile = 'balanced', quality) {
    const resolved = resolveEncodeProfile(format, profile, quality)
    return this.toBufferWithMetrics(resolved.format, resolved.quality, resolved.fastMode)
  }

  p.toFileProfile = function toFileProfile(path, format, profile = 'balanced', quality) {
    const resolved = resolveEncodeProfile(format, profile, quality)
    return this.toFile(path, resolved.format, resolved.quality, resolved.fastMode)
  }

  p.toBufferTargetBytes = async function toBufferTargetBytes(format, options) {
    const result = await encodeWithinByteBudget(this, format, options)
    return {
      data: result.data,
      quality: result.quality,
      bytesOut: result.bytesOut,
      budgetMet: result.budgetMet,
      targetBytes: result.targetBytes,
      metrics: result.metrics,
    }
  }

  p.toFileTargetBytes = async function toFileTargetBytes(path, format, options) {
    const result = await encodeWithinByteBudget(this, format, options)
    const written = await this.toFileWithMetrics(path, result.format, result.quality, result.fastMode)
    return {
      bytesWritten: written.bytesWritten,
      quality: result.quality,
      budgetMet: written.bytesWritten <= result.targetBytes,
      targetBytes: result.targetBytes,
      metrics: written.metrics,
    }
  }
}
`;

const dtsExtraBlock = `

type CaseInsensitive<T extends string> = T | Uppercase<T> | Capitalize<T>
type CanonicalSupportedInputFormat = 'jpeg' | 'jpg' | 'png' | 'webp'
type CanonicalInputFormat = CanonicalSupportedInputFormat
type CanonicalOutputFormat = CanonicalInputFormat | 'jpg' | 'avif'
type CanonicalPresetName = 'thumbnail' | 'avatar' | 'hero' | 'social'

export type InputFormat = CaseInsensitive<CanonicalSupportedInputFormat> | (string & {})
export type OutputFormat = CaseInsensitive<CanonicalOutputFormat>
export type TargetBytesFormat = CaseInsensitive<'jpeg' | 'jpg' | 'webp' | 'avif'>
export type PresetName = CaseInsensitive<CanonicalPresetName>
export type ResizeFit = 'inside' | 'cover' | 'fill'
export type FirewallPolicy = 'strict' | 'lenient'
export type EncodeProfile = 'size-first' | 'balanced' | 'speed-first'

export interface ResolvedEncodeProfile {
  format: CanonicalOutputFormat
  quality?: number
  fastMode: boolean
}

export interface ImageEngine {
  toBufferProfile(format: OutputFormat, profile?: EncodeProfile, quality?: number | undefined | null): Promise<Buffer>
  toBufferWithMetricsProfile(format: OutputFormat, profile?: EncodeProfile, quality?: number | undefined | null): Promise<OutputWithMetrics>
  toFileProfile(path: string, format: OutputFormat, profile?: EncodeProfile, quality?: number | undefined | null): Promise<number>
  toBufferTargetBytes(format: TargetBytesFormat, options: TargetBytesOptions): Promise<BufferTargetBytesResult>
  toFileTargetBytes(path: string, format: TargetBytesFormat, options: TargetBytesOptions): Promise<FileTargetBytesResult>
}

export declare function getErrorCategory(err: unknown): ErrorCategory | null

export declare function createStreamingPipeline(options: {
  format?: OutputFormat
  quality?: number
  ops?: Array<{
    op: 'resize' | 'rotate' | 'flipH' | 'flipV' | 'grayscale' | 'autoOrient'
    width?: number
    height?: number
    fit?: ResizeFit
    degrees?: number
    enabled?: boolean
  }>
  ImageEngine?: typeof ImageEngine
  onMetrics?: (metrics: ProcessingMetrics) => void
}): {
  writable: NodeJS.WritableStream
  readable: NodeJS.ReadableStream
}

export interface TargetBytesOptions {
  /** Target file size in bytes */
  targetBytes: number
  /** Alias for targetBytes */
  maxBytes?: number
  /** Minimum quality to try (default: 30) */
  minQuality?: number
  /** Maximum quality to try (default: 100) */
  maxQuality?: number
  /** Enable fast encoding mode */
  fastMode?: boolean
  /** Behaviour when budget cannot be met: 'best-effort' (default) or 'strict' */
  qualityFloorPolicy?: 'best-effort' | 'strict'
}

export interface BufferTargetBytesResult {
  /** Encoded image data */
  data: Buffer
  /** Quality level that was used */
  quality: number
  /** Actual output size in bytes */
  bytesOut: number
  /** Whether the byte budget was met */
  budgetMet: boolean
  /** Requested target bytes */
  targetBytes: number
  /** Processing metrics */
  metrics: ProcessingMetrics
}

export interface FileTargetBytesResult {
  /** Bytes written to disk */
  bytesWritten: number
  /** Quality level that was used */
  quality: number
  /** Whether the byte budget was met */
  budgetMet: boolean
  /** Requested target bytes */
  targetBytes: number
  /** Processing metrics */
  metrics: ProcessingMetrics
}

export declare function resolveEncodeProfile(format: OutputFormat, profile?: EncodeProfile, quality?: number | undefined | null): ResolvedEncodeProfile
`;

function replaceIfNeeded(content, searchValue, replaceValue) {
  if (content.includes(replaceValue)) return content;
  if (!content.includes(searchValue)) {
    // Type defs may be absent when napi build skips generation (e.g. cached
    // ASan builds). Warn instead of crashing so the build can continue.
    console.warn(`Warning: expected snippet not found in index.d.ts (skipped): ${searchValue.slice(0, 60)}…`);
    return content;
  }
  return content.replace(searchValue, replaceValue);
}

function patchIndexJs() {
  let content = fs.readFileSync(indexJsPath, 'utf8');
  if (!content.includes('module.exports.getErrorCategory = getErrorCategory')) {
    content += jsHelperBlock;
    fs.writeFileSync(indexJsPath, content);
  }
}

function patchIndexDts() {
  let content = fs.readFileSync(indexDtsPath, 'utf8');

  content = replaceIfNeeded(content, '  preset(name: string): PresetResult', '  preset(name: PresetName): PresetResult');
  content = replaceIfNeeded(content, '  toBuffer(format: string, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<Buffer>', '  toBuffer(format: OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<Buffer>');
  content = replaceIfNeeded(content, '  toBufferWithPreset(presetName: string): Promise<Buffer>', '  toBufferWithPreset(presetName: PresetName): Promise<Buffer>');
  content = replaceIfNeeded(content, '  toBufferWithMetrics(format: string, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<OutputWithMetrics>', '  toBufferWithMetrics(format: OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<OutputWithMetrics>');
  content = replaceIfNeeded(content, '  toBufferWithMetricsPreset(presetName: string): Promise<OutputWithMetrics>', '  toBufferWithMetricsPreset(presetName: PresetName): Promise<OutputWithMetrics>');
  content = replaceIfNeeded(content, '  toFile(path: string, format: string, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<number>', '  toFile(path: string, format: OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<number>');
  content = replaceIfNeeded(content, '  toFileWithPreset(path: string, presetName: string): Promise<number>', '  toFileWithPreset(path: string, presetName: PresetName): Promise<number>');
  content = replaceIfNeeded(content, '  processBatch(inputs: Array<string>, outputDir: string, optionsOrFormat: BatchOptions | string, quality?: number | undefined | null, fastMode?: boolean | undefined | null, concurrency?: number | undefined | null): Promise<BatchResult[]>', '  processBatch(inputs: Array<string>, outputDir: string, optionsOrFormat: BatchOptions | OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null, concurrency?: number | undefined | null): Promise<BatchResult[]>');
  content = replaceIfNeeded(content, '  /** Output format ("jpeg", "png", "webp", "avif") */\n  format: string', '  /** Output format ("jpeg", "jpg", "png", "webp", "avif") */\n  format: OutputFormat');
  content = replaceIfNeeded(content, '  /** Detected format (jpeg, png, webp, gif, etc.) */\n  format?: string', '  /** Detected input format from runtime inspection (for example: jpeg, jpg, png, webp, gif) */\n  format?: InputFormat');
  content = replaceIfNeeded(content, '  /** Recommended output format */\n  format: string', '  /** Recommended output format */\n  format: CanonicalOutputFormat');
  content = replaceIfNeeded(content, 'export interface PresetResult {\n  /** Recommended output format */\n  format: string', 'export interface PresetResult {\n  /** Recommended output format */\n  format: CanonicalOutputFormat');
  content = replaceIfNeeded(content, '  /** Detected input format (lowercase: jpeg, png, webp, avif, etc.) */\n  formatIn?: string', '  /** Detected input format reported by runtime metrics */\n  formatIn?: InputFormat');
  content = replaceIfNeeded(content, '  /** Output format */\n  formatOut: string', '  /** Output format */\n  formatOut: CanonicalOutputFormat');
  content = replaceIfNeeded(content, '  toFileWithMetrics(path: string, format: string,', '  toFileWithMetrics(path: string, format: OutputFormat,');
  content = replaceIfNeeded(content, '  fit?: string', '  fit?: ResizeFit');
  content = replaceIfNeeded(content, '  policy?: string', '  policy?: FirewallPolicy');
  content = replaceIfNeeded(content, 'export declare function supportedInputFormats(): Array<string>', 'export declare function supportedInputFormats(): Array<CanonicalInputFormat>');
  content = replaceIfNeeded(content, 'export declare function supportedOutputFormats(): Array<string>', 'export declare function supportedOutputFormats(): Array<CanonicalOutputFormat>');

  if (!content.includes('export declare function getErrorCategory(err: unknown): ErrorCategory | null')) {
    content += dtsExtraBlock;
  }

  fs.writeFileSync(indexDtsPath, content);
}

patchIndexJs();
patchIndexDts();
console.log('Applied postbuild fixups to index.js and index.d.ts.');
