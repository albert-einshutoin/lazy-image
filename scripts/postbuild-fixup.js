const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const root = path.resolve(__dirname, '..');
const indexJsPath = path.join(root, 'index.js');
const indexDtsPath = path.join(root, 'index.d.ts');
const backupDir = path.join(root, '.tmp', 'postbuild-fixup');
const indexDtsBackupPath = path.join(backupDir, 'index.d.ts');

const dtsExtraBlock = `

type CaseInsensitive<T extends string> = T | Uppercase<T> | Capitalize<T>
type CanonicalSupportedInputFormat = 'jpeg' | 'jpg' | 'png' | 'webp'
type CanonicalInputFormat = CanonicalSupportedInputFormat
// CanonicalInputFormat already contains 'jpg'; only 'avif' is output-exclusive.
type CanonicalOutputFormat = CanonicalInputFormat | 'avif'
type CanonicalPresetName = 'thumbnail' | 'avatar' | 'hero' | 'social'

export type InputFormat = CaseInsensitive<CanonicalSupportedInputFormat> | (string & {})
export type OutputFormat = CaseInsensitive<CanonicalOutputFormat>
export type TargetBytesFormat = CaseInsensitive<'jpeg' | 'jpg' | 'webp' | 'avif'>
export type PresetName = CaseInsensitive<CanonicalPresetName>
export type ResizeFit = 'inside' | 'cover' | 'fill'
export type FirewallPolicy = 'strict' | 'lenient' | 'public-upload'
export type IccOutcome = 'absent' | 'preserved' | 'unsafe-stripped' | 'policy-stripped' | 'unsupported'
export type EncodeProfile = 'size-first' | 'balanced' | 'speed-first'
export type ArtifactFormat = 'jpeg' | 'webp' | 'avif'

export interface ArtifactByteBudget {
  readonly width: number
  readonly format: ArtifactFormat
  readonly maxBytes: number
}

export interface PublicUploadPolicy {
  readonly preset?: 'publicUpload'
  readonly widths?: readonly number[]
  readonly formats?: readonly ArtifactFormat[]
  readonly budgets?: readonly ArtifactByteBudget[]
  readonly placeholder?: boolean
}

export interface ArtifactSourceFacts {
  readonly sha256: string
  readonly bytes: number
  readonly format: 'jpeg' | 'jpg' | 'png' | 'webp'
  readonly width: number
  readonly height: number
  readonly hasAlpha: boolean
  readonly isAnimated: boolean
  readonly orientationKnown: true
  readonly orientation: number | null
}

export interface ArtifactPlanSource {
  readonly sha256: string
  readonly bytes: number
  readonly detectedFormat: 'jpeg' | 'png' | 'webp'
  readonly encodedWidth: number
  readonly encodedHeight: number
  readonly displayWidth: number
  readonly displayHeight: number
  readonly hasAlpha: boolean
  readonly isAnimated: false
  readonly orientationKnown: true
  readonly orientation: number | null
  readonly orientationApplied: boolean
}

export interface ArtifactPlanPolicy {
  readonly preset: 'publicUpload'
  readonly widths: readonly number[]
  readonly formats: readonly ArtifactFormat[]
  readonly budgets: readonly ArtifactByteBudget[]
  readonly placeholder: boolean
}

export interface ArtifactPlanItem {
  readonly id: string
  readonly path: string
  readonly format: ArtifactFormat
  readonly width: number
  readonly quality: number
  readonly targetBytes?: number
}

export interface ArtifactPlan {
  readonly schemaVersion: 1
  readonly compilerFingerprint: string
  readonly policyFingerprint: string
  readonly cacheKey: string
  readonly source: ArtifactPlanSource
  readonly policy: ArtifactPlanPolicy
  readonly artifacts: readonly ArtifactPlanItem[]
  readonly delivery: {
    readonly srcsets: readonly { readonly format: ArtifactFormat; readonly value: string }[]
  }
  readonly placeholder: {
    readonly id: 'placeholder'
    readonly path: 'placeholder.webp'
    readonly format: 'webp'
    readonly width: 16
    readonly quality: 20
  } | null
}

export declare function compileArtifactPlan(
  source: ArtifactSourceFacts,
  policy: PublicUploadPolicy | undefined,
  compilerFingerprint: string,
): ArtifactPlan

export interface CompileImageOptions {
  readonly inputPath: string
  readonly outputDir: string
  readonly policy: PublicUploadPolicy
  readonly signal?: AbortSignal
}

export interface ImageArtifactManifest {
  readonly schemaVersion: 1
  readonly compiler: {
    readonly name: '@alberteinshutoin/lazy-image'
    readonly version: string
    readonly platform: string
    readonly arch: string
    readonly fingerprint: string
  }
  readonly policy: {
    readonly preset: 'publicUpload'
    readonly fingerprint: string
    readonly widths: readonly number[]
    readonly formats: readonly ArtifactFormat[]
    readonly placeholder: boolean
  }
  readonly source: {
    readonly sha256: string
    readonly bytes: number
    readonly detectedFormat: 'jpeg' | 'png' | 'webp'
    readonly encodedWidth: number
    readonly encodedHeight: number
    readonly displayWidth: number
    readonly displayHeight: number
    readonly hasAlpha: boolean
    readonly orientation: number | null
    readonly orientationApplied: boolean
  }
  readonly metadata: {
    readonly mode: 'privacySafe'
    readonly exif: 'stripped'
    readonly gps: 'stripped'
    readonly xmp: 'stripped'
    readonly comments: 'stripped'
    readonly unknownAncillary: 'stripped'
    readonly artifactIccPolicy: 'preserve-if-safe'
    readonly placeholderIccPolicy: 'strip'
  }
  readonly artifacts: readonly {
    readonly id: string
    readonly path: string
    readonly format: ArtifactFormat
    readonly width: number
    readonly height: number
    readonly bytes: number
    readonly quality: number
    readonly targetBytes?: number
    readonly iccOutcome: IccOutcome
    readonly sha256: string
  }[]
  readonly delivery: {
    readonly srcsets: readonly { readonly format: ArtifactFormat; readonly value: string }[]
  }
  readonly placeholder?: {
    readonly path: 'placeholder.webp'
    readonly dataUrl: string
    readonly format: 'webp'
    readonly width: number
    readonly height: number
    readonly bytes: number
    readonly iccOutcome: 'stripped-for-placeholder'
    readonly sha256: string
  }
  readonly cacheKey: string
}

export interface ArtifactCompilationError extends Error {
  readonly phase: 'preflight' | 'planning' | 'processing' | 'verification' | 'commit' | 'cleanup'
  readonly artifactId?: string
  readonly errorCode?: string
  readonly category?: ErrorCategory
  readonly errorCategory?: ErrorCategory
  readonly code?: string
  readonly recoveryHint?: string
  readonly cause?: unknown
  readonly cleanupError?: unknown
}

export declare function compileImage(options: CompileImageOptions): Promise<ImageArtifactManifest>

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
  // Note: encode() / encodeToFile() are declared on the ImageEngine class
  // (see patchIndexDts patches that retype them to EncodeOptionsInput) and
  // are deliberately NOT re-declared here to avoid duplicate signatures.
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

export interface BatchProgressEvent {
  /** Completed file count after the latest chunk finishes */
  completed: number
  /** Total input file count */
  total: number
  /** Results returned by the latest processBatch chunk */
  lastChunk: BatchResult[]
  /** Cumulative failed result count through the latest chunk */
  failed: number
}

export interface ProcessBatchChunkedOptions extends BatchOptions {
  /** Number of files per native processBatch call. Default: 16. */
  chunkSize?: number
  /** Called after each chunk. Throwing is logged and does not stop processing. */
  onProgress?: (event: BatchProgressEvent) => void
  /** Checked before each chunk starts. Aborted signals reject with AbortError. */
  signal?: AbortSignal
}

export declare function processBatchChunked(
  inputs: string[],
  outputDir: string,
  options: ProcessBatchChunkedOptions,
): Promise<BatchResult[]>

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
  /** Fail when the target cannot be met (alias for qualityFloorPolicy: 'strict') */
  strict?: boolean
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

function readTextIfExists(filePath) {
  try {
    return fs.readFileSync(filePath, 'utf8');
  } catch (err) {
    if (err && err.code === 'ENOENT') return null;
    throw err;
  }
}

function readCommittedIndexDts() {
  try {
    return execFileSync('git', ['show', 'HEAD:index.d.ts'], {
      cwd: root,
      encoding: 'utf8',
      stdio: ['ignore', 'pipe', 'ignore'],
    });
  } catch {
    return null;
  }
}

function prepareBuild() {
  let content = readTextIfExists(indexDtsPath);

  if (!content || content.trim().length === 0) {
    content = readCommittedIndexDts();
  }

  if (!content || content.trim().length === 0) {
    throw new Error('postbuild-fixup: cannot prepare index.d.ts backup; current and committed files are empty or unavailable');
  }

  fs.mkdirSync(backupDir, { recursive: true });
  fs.writeFileSync(indexDtsBackupPath, content);
  console.log('Prepared postbuild index.d.ts backup.');
}

function restoreDtsIfGeneratedEmpty(content) {
  if (content.trim().length > 0) return content;

  // NAPI-RS can write an empty index.d.ts when no intermediate typedef files
  // are emitted. Restoring the last public declarations prevents a clean
  // build from publishing an empty type surface while still letting the
  // explicit patch checks below catch real signature drift.
  const backup = readTextIfExists(indexDtsBackupPath) || readCommittedIndexDts();
  if (!backup || backup.trim().length === 0) {
    throw new Error('postbuild-fixup: generated index.d.ts is empty and no usable backup is available');
  }
  return backup;
}

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

function patchImageMetadataFormat(content) {
  return replaceIfNeeded(
    content,
    '  /** Detected supported input format. */\n  format?: string',
    '  /** Detected supported input format. */\n  format?: InputFormat',
  );
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
const { compileArtifactPlan } = require('./lib/artifact-policy')
const { compileImage } = require('./lib/artifact-compiler')
const _getErrorCategory = helpers.createGetErrorCategory(nativeBinding.ErrorCategory, nativeBinding.ErrorCode)
const _processBatchChunked = helpers.createProcessBatchChunked(nativeBinding.ImageEngine)
module.exports.getErrorCategory = _getErrorCategory
module.exports.processBatchChunked = _processBatchChunked
module.exports.createStreamingPipeline = createStreamingPipeline
module.exports.resolveEncodeProfile = helpers.resolveEncodeProfile
module.exports.compileArtifactPlan = compileArtifactPlan
module.exports.compileImage = compileImage
if (nativeBinding && nativeBinding.ImageEngine) {
  helpers.attachPrototypeMethods(nativeBinding.ImageEngine)
}
`;
    fs.writeFileSync(indexJsPath, content);
  }
}

function patchIndexDts() {
  let content = restoreDtsIfGeneratedEmpty(fs.readFileSync(indexDtsPath, 'utf8'));

  content = replaceIfNeeded(content, '  preset(name: string): PresetResult', '  preset(name: PresetName): PresetResult');
  content = replaceIfNeeded(content, '  toBuffer(format: string, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<Buffer>', '  toBuffer(format: OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<Buffer>');
  content = replaceIfNeeded(content, '  toBufferWithPreset(presetName: string): Promise<Buffer>', '  toBufferWithPreset(presetName: PresetName): Promise<Buffer>');
  content = replaceIfNeeded(content, '  toBufferWithMetrics(format: string, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<OutputWithMetrics>', '  toBufferWithMetrics(format: OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<OutputWithMetrics>');
  content = replaceIfNeeded(content, '  toBufferWithMetricsPreset(presetName: string): Promise<OutputWithMetrics>', '  toBufferWithMetricsPreset(presetName: PresetName): Promise<OutputWithMetrics>');
  content = replaceIfNeeded(content, '  toFile(path: string, format: string, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<number>', '  toFile(path: string, format: OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null): Promise<number>');
  content = replaceIfNeeded(content, '  toFileWithPreset(path: string, presetName: string): Promise<number>', '  toFileWithPreset(path: string, presetName: PresetName): Promise<number>');
  content = replaceIfNeeded(content, '  processBatch(inputs: Array<string>, outputDir: string, optionsOrFormat: BatchOptions | string, quality?: number | undefined | null, fastMode?: boolean | undefined | null, concurrency?: number | undefined | null): Promise<BatchResult[]>', '  processBatch(inputs: Array<string>, outputDir: string, optionsOrFormat: BatchOptions | OutputFormat, quality?: number | undefined | null, fastMode?: boolean | undefined | null, concurrency?: number | undefined | null): Promise<BatchResult[]>');
  content = replaceIfNeeded(content, '  /** Output format ("jpeg", "png", "webp", "avif") */\n  format: string', '  /** Output format ("jpeg", "jpg", "png", "webp", "avif") */\n  format: OutputFormat');
  content = patchImageMetadataFormat(content);
  content = replaceIfNeeded(content, '  /** Recommended output format */\n  format: string', '  /** Recommended output format */\n  format: CanonicalOutputFormat');
  content = replaceIfNeeded(content, 'export interface PresetResult {\n  /** Recommended output format */\n  format: string', 'export interface PresetResult {\n  /** Recommended output format */\n  format: CanonicalOutputFormat');
  content = replaceIfNeeded(content, '  /** Detected input format (lowercase: jpeg, png, webp, avif, etc.) */\n  formatIn?: string', '  /** Detected input format reported by runtime metrics */\n  formatIn?: InputFormat');
  content = replaceIfNeeded(content, '  /** Output format */\n  formatOut: string', '  /** Output format */\n  formatOut: CanonicalOutputFormat');
  content = replaceIfNeeded(content, '  iccOutcome: string', '  iccOutcome: IccOutcome');
  content = replaceIfNeeded(content, '  toFileWithMetrics(path: string, format: string,', '  toFileWithMetrics(path: string, format: OutputFormat,');
  content = replaceIfNeeded(content, '  encode(options: EncodeOptions): Promise<EncodeResult>', '  encode(options: EncodeOptionsInput): Promise<EncodeResult>');
  content = replaceIfNeeded(content, '  encodeToFile(path: string, options: EncodeOptions): Promise<FileEncodeResult>', '  encodeToFile(path: string, options: EncodeOptionsInput): Promise<FileEncodeResult>');
  // Mark the NAPI-generated `EncodeOptions` interface as deprecated.
  // The exported type uses `format?: string` (weak typing); consumers should
  // import `EncodeOptionsInput` instead, which uses `format?: OutputFormat`.
  // We patch the JSDoc rather than removing the export because the interface
  // is still the NAPI ABI type for `encode()` / `encodeToFile()` at runtime.
  content = replaceIfNeeded(
    content,
    '/** Options object for the unified `encode()` / `encodeToFile()` methods. */\nexport interface EncodeOptions {',
    '/**\n * Options object for the unified `encode()` / `encodeToFile()` methods.\n *\n * @deprecated Use `EncodeOptionsInput` instead — this interface uses `format?: string`,\n * whereas `EncodeOptionsInput` is strongly typed (`format?: OutputFormat`). The class\n * methods on `ImageEngine` accept `EncodeOptionsInput`; this type is retained only for\n * the NAPI ABI signature.\n */\nexport interface EncodeOptions {',
  );
  content = replaceIfNeeded(content, '  fit?: string', '  fit?: ResizeFit');
  content = replaceIfNeeded(content, '  policy?: string', '  policy?: FirewallPolicy');
  content = replaceIfNeeded(content, 'export declare function supportedInputFormats(): Array<string>', 'export declare function supportedInputFormats(): Array<CanonicalInputFormat>');
  content = replaceIfNeeded(content, 'export declare function supportedOutputFormats(): Array<string>', 'export declare function supportedOutputFormats(): Array<CanonicalOutputFormat>');

  if (!content.includes('export declare function getErrorCategory(err: unknown): ErrorCategory | null')) {
    content += dtsExtraBlock;
  }

  fs.writeFileSync(indexDtsPath, content);
}

if (require.main === module) {
  if (process.argv.includes('--prepare')) {
    prepareBuild();
  } else {
    patchIndexJs();
    patchIndexDts();
    console.log('Applied postbuild fixups to index.js and index.d.ts.');
  }
}

module.exports = { patchImageMetadataFormat };
