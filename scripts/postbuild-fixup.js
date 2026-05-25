const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const indexJsPath = path.join(root, 'index.js');
const indexDtsPath = path.join(root, 'index.d.ts');

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
  encode(options: EncodeOptionsInput): Promise<EncodeResult>
  encodeToFile(path: string, options: EncodeOptionsInput): Promise<FileEncodeResult>
}

export interface EncodeOptionsInput {
  /** Output format: "jpeg", "jpg", "png", "webp", "avif" */
  format?: OutputFormat
  /** Quality 1-100 for lossy formats. Omit for PNG. */
  quality?: number
  /** If true, uses faster JPEG encoding (2-4x faster, slightly larger). Default: false. */
  fastMode?: boolean
  /** Preset name. When specified, format/quality/fastMode are ignored. */
  preset?: PresetName
  /** If true, include ProcessingMetrics in the result. Default: false. */
  metrics?: boolean
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
  onCleanupError?: (err: Error, target: string) => void
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
    throw new Error(
      `postbuild-fixup: replacement target not found in index.d.ts — the NAPI-generated ` +
      `type definitions may have changed. Search string (first 80 chars): ` +
      `${JSON.stringify(searchValue.slice(0, 80))}`
    );
  }
  return content.replaceAll(searchValue, replaceValue);
}

function patchIndexJs() {
  let content = fs.readFileSync(indexJsPath, 'utf8');
  if (!content.includes("require('./lib/helpers')")) {
    // Remove any previously appended helper block (from older postbuild runs)
    // by trimming everything after the NAPI-generated module.exports block.
    // The canonical NAPI footer ends with the last `module.exports.<x> = ...` line.
    content += `

// High-level helpers and streaming API (extracted to lib/helpers.js)
const { createStreamingPipeline } = require('./streaming/pipeline')
const helpers = require('./lib/helpers')
const _getErrorCategory = helpers.createGetErrorCategory(nativeBinding.ErrorCategory, nativeBinding.ErrorCode)
module.exports.getErrorCategory = _getErrorCategory
module.exports.createStreamingPipeline = createStreamingPipeline
module.exports.resolveEncodeProfile = helpers.resolveEncodeProfile
if (nativeBinding && nativeBinding.ImageEngine) {
  helpers.attachPrototypeMethods(nativeBinding.ImageEngine)
}
`;
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
  content = replaceIfNeeded(content, '  encode(options: EncodeOptions): Promise<EncodeResult>', '  encode(options: EncodeOptionsInput): Promise<EncodeResult>');
  content = replaceIfNeeded(content, '  encodeToFile(path: string, options: EncodeOptions): Promise<FileEncodeResult>', '  encodeToFile(path: string, options: EncodeOptionsInput): Promise<FileEncodeResult>');
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
