'use strict';

const crypto = require('node:crypto');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const zlib = require('node:zlib');

const { compileArtifactPlan } = require('./artifact-policy');

const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const MAX_PIXELS = 40_000_000;
const PLACEHOLDER_MAX_BYTES = 1024 * 1024;
const MAX_METADATA_BUFFER = 1024 * 1024;
const MAX_ICC_BYTES = 512 * 1024;
const STAGING_MARKER = '.lazy-image-staging';
const PACKAGE_NAME = '@alberteinshutoin/lazy-image';
const PACKAGE_VERSION = require('../package.json').version;
const ICC_OUTCOMES = new Set(['absent', 'preserved', 'unsafe-stripped', 'policy-stripped', 'unsupported']);

const ERROR_CATEGORY = {
  UserError: 0,
  CodecError: 1,
  ResourceLimit: 2,
  InternalBug: 3,
};

const OPTION_KEYS = new Set(['inputPath', 'outputDir', 'policy', 'signal']);

class ArtifactCompilationError extends Error {
  constructor(message, details = {}) {
    super(message);
    this.name = 'ArtifactCompilationError';
    this.phase = details.phase;
    if (details.artifactId !== undefined) this.artifactId = details.artifactId;
    if (details.errorCode !== undefined) this.errorCode = details.errorCode;
    if (details.category !== undefined) {
      this.category = details.category;
      this.errorCategory = details.category;
    }
    if (details.recoveryHint !== undefined) this.recoveryHint = details.recoveryHint;
    if (details.cause !== undefined) this.cause = details.cause;
    this.code = details.code || categoryCode(details.category);
  }
}

function categoryCode(category) {
  switch (category) {
    case ERROR_CATEGORY.UserError: return 'LAZY_IMAGE_USER_ERROR';
    case ERROR_CATEGORY.CodecError: return 'LAZY_IMAGE_CODEC_ERROR';
    case ERROR_CATEGORY.ResourceLimit: return 'LAZY_IMAGE_RESOURCE_LIMIT';
    case ERROR_CATEGORY.InternalBug: return 'LAZY_IMAGE_INTERNAL_BUG';
    default: return undefined;
  }
}

function abortError(phase, cause) {
  const error = new Error('Image artifact compilation aborted');
  error.name = 'AbortError';
  error.code = 'ABORT_ERR';
  error.phase = phase;
  if (cause !== undefined) error.cause = cause;
  return error;
}

function isAbortError(error) {
  return Boolean(error && (error.name === 'AbortError' || error.code === 'ABORT_ERR'));
}

function throwIfAborted(signal, phase) {
  if (signal && signal.aborted) throw abortError(phase);
}

function ownData(value, key, name, required = true) {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  if (!descriptor) {
    if (required) throw inputError(`${name}.${key} is required`);
    return undefined;
  }
  if (!Object.hasOwn(descriptor, 'value')) {
    throw inputError(`${name}.${key} must be an own data property`);
  }
  return descriptor.value;
}

function inputError(message) {
  return structuredError(message, 'E400', ERROR_CATEGORY.UserError, 'Provide inputPath, outputDir, and a valid publicUpload policy.');
}

function structuredError(message, errorCode, category, recoveryHint, cause) {
  const error = new Error(message);
  error.errorCode = errorCode;
  error.category = category;
  error.errorCategory = category;
  error.code = categoryCode(category);
  error.recoveryHint = recoveryHint;
  if (cause !== undefined) error.cause = cause;
  return error;
}

function validateOptions(options) {
  if (options === null || typeof options !== 'object' || Array.isArray(options)) {
    throw inputError('compileImage options must be an object');
  }
  const prototype = Object.getPrototypeOf(options);
  if (prototype !== Object.prototype && prototype !== null) {
    throw inputError('compileImage options must be a plain object');
  }
  for (const key of Reflect.ownKeys(options)) {
    if (typeof key !== 'string' || !OPTION_KEYS.has(key)) {
      throw inputError(`unknown compileImage option: ${String(key)}`);
    }
  }

  const inputPath = ownData(options, 'inputPath', 'options');
  const outputDir = ownData(options, 'outputDir', 'options');
  const policy = ownData(options, 'policy', 'options');
  const signal = ownData(options, 'signal', 'options', false);

  for (const [value, name] of [[inputPath, 'inputPath'], [outputDir, 'outputDir']]) {
    if (typeof value !== 'string' || value.length === 0 || value.includes('\0')) {
      throw inputError(`options.${name} must be a non-empty path string`);
    }
  }
  if (policy === null || typeof policy !== 'object' || Array.isArray(policy)) {
    throw inputError('options.policy must be a publicUpload policy object');
  }
  if (signal !== undefined && (signal === null || typeof signal !== 'object' || typeof signal.aborted !== 'boolean'
    || typeof signal.addEventListener !== 'function' || typeof signal.removeEventListener !== 'function')) {
    throw inputError('options.signal must be an AbortSignal');
  }
  return { inputPath, outputDir, policy, signal };
}

function wrapError(error, phase, artifactId, fallback = {}) {
  if (error instanceof ArtifactCompilationError) return error;
  if (isAbortError(error)) return error;
  const errorCode = error && error.errorCode !== undefined ? error.errorCode : fallback.errorCode;
  const category = normalizeCategory(error && error.category !== undefined
    ? error.category
    : (error && error.errorCategory !== undefined ? error.errorCategory : fallback.category));
  const recoveryHint = error && error.recoveryHint ? error.recoveryHint : fallback.recoveryHint;
  const wrapped = new ArtifactCompilationError(
    `Image artifact compilation failed during ${phase}${artifactId ? ` (${artifactId})` : ''}`,
    {
      phase,
      artifactId,
      errorCode,
      category,
      recoveryHint,
      cause: error,
      code: error && error.code ? error.code : categoryCode(category),
    },
  );
  return wrapped;
}

function normalizeCategory(category) {
  if (typeof category === 'number') return category;
  if (typeof category === 'string' && Object.hasOwn(ERROR_CATEGORY, category)) {
    return ERROR_CATEGORY[category];
  }
  return category;
}

function statIdentity(stat) {
  return {
    dev: stat.dev,
    ino: stat.ino,
    size: stat.size,
    mtimeMs: stat.mtimeMs,
    ctimeMs: stat.ctimeMs,
  };
}

function sameIdentity(left, right) {
  return left.dev === right.dev
    && left.ino === right.ino
    && left.size === right.size
    && left.mtimeMs === right.mtimeMs
    && left.ctimeMs === right.ctimeMs;
}

async function lstatRegular(filePath, fallback) {
  let stat;
  try {
    stat = await fsp.lstat(filePath);
  } catch (error) {
    const code = error && error.code === 'ENOENT' ? 'E100' : fallback.errorCode;
    const category = code === 'E100' ? ERROR_CATEGORY.UserError : fallback.category;
    throw structuredError(
      code === 'E100' ? 'Input file was not found' : 'File access failed',
      code,
      category,
      code === 'E100' ? 'Verify the input path and ensure the file exists.' : fallback.recoveryHint,
      error,
    );
  }
  if (stat.isSymbolicLink()) {
    throw structuredError('Symbolic-link paths are not accepted', fallback.errorCode, fallback.category, fallback.recoveryHint);
  }
  if (!stat.isFile()) {
    throw structuredError('The path must refer to a regular file', fallback.errorCode, fallback.category, fallback.recoveryHint);
  }
  return stat;
}

async function hashHandle(handle, signal, maxBytes, phase = 'verification') {
  throwIfAborted(signal, phase);
  const hash = crypto.createHash('sha256');
  const buffer = Buffer.allocUnsafe(64 * 1024);
  let bytes = 0;
  let position = 0;
  while (true) {
    throwIfAborted(signal, phase);
    const { bytesRead } = await handle.read(buffer, 0, buffer.length, position);
    if (bytesRead === 0) break;
    bytes += bytesRead;
    if (maxBytes !== undefined && bytes > maxBytes) {
      throw structuredError('Input exceeds the public-upload byte limit', 'E121', ERROR_CATEGORY.ResourceLimit, 'Use an input no larger than 32 MiB.');
    }
    hash.update(buffer.subarray(0, bytesRead));
    position += bytesRead;
  }
  return { bytes, sha256: hash.digest('hex') };
}

async function hashRegularFile(filePath, signal, phase = 'verification') {
  throwIfAborted(signal, phase);
  const before = await fsp.lstat(filePath);
  if (!before.isFile() || before.isSymbolicLink()) throw new Error('file is not a regular file');
  let handle;
  try {
    handle = await fsp.open(filePath, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
    const opened = await handle.stat();
    if (!opened.isFile() || !sameIdentity(statIdentity(before), statIdentity(opened))) throw new Error('file changed before hashing');
    const digest = await hashHandle(handle, signal, undefined, phase);
    const after = await handle.stat();
    if (!sameIdentity(statIdentity(before), statIdentity(after))) throw new Error('file changed during hashing');
    return digest;
  } finally {
    if (handle) await handle.close().catch(() => {});
  }
}

async function writeHandle(handle, data) {
  let offset = 0;
  while (offset < data.length) {
    const { bytesWritten } = await handle.write(data, offset, data.length - offset);
    if (bytesWritten === 0) throw new Error('snapshot write made no progress');
    offset += bytesWritten;
  }
}

async function createSourceSnapshot(inputPath, signal) {
  throwIfAborted(signal, 'preflight');
  const pathStat = await lstatRegular(inputPath, {
    errorCode: 'E101',
    category: ERROR_CATEGORY.ResourceLimit,
    recoveryHint: 'Verify input permissions and availability.',
  });
  if (pathStat.size > MAX_INPUT_BYTES) {
    throw structuredError('Input exceeds the public-upload byte limit', 'E121', ERROR_CATEGORY.ResourceLimit, 'Use an input no larger than 32 MiB.');
  }

  let sourceHandle;
  let snapshotDir;
  let snapshotHandle;
  try {
    const flags = fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0);
    sourceHandle = await fsp.open(inputPath, flags);
    const openedStat = await sourceHandle.stat();
    if (!openedStat.isFile() || !sameIdentity(statIdentity(pathStat), statIdentity(openedStat))) {
      throw structuredError('Input changed while it was being opened', 'E101', ERROR_CATEGORY.ResourceLimit, 'Keep the input unchanged and retry.');
    }

    snapshotDir = await fsp.mkdtemp(path.join(os.tmpdir(), 'lazy-image-source-'));
    await fsp.chmod(snapshotDir, 0o700);
    const snapshotPath = path.join(snapshotDir, 'source.bin');
    snapshotHandle = await fsp.open(snapshotPath, 'wx', 0o600);
    const hash = crypto.createHash('sha256');
    const buffer = Buffer.allocUnsafe(64 * 1024);
    let bytes = 0;
    let position = 0;
    while (true) {
      throwIfAborted(signal, 'preflight');
      const { bytesRead } = await sourceHandle.read(buffer, 0, buffer.length, position);
      if (bytesRead === 0) break;
      bytes += bytesRead;
      if (bytes > MAX_INPUT_BYTES) {
        throw structuredError('Input exceeds the public-upload byte limit', 'E121', ERROR_CATEGORY.ResourceLimit, 'Use an input no larger than 32 MiB.');
      }
      const chunk = buffer.subarray(0, bytesRead);
      hash.update(chunk);
      await writeHandle(snapshotHandle, chunk);
      position += bytesRead;
    }
    const finalSourceStat = await sourceHandle.stat();
    if (!sameIdentity(statIdentity(pathStat), statIdentity(finalSourceStat))) {
      throw structuredError('Input changed while it was being snapshotted', 'E101', ERROR_CATEGORY.ResourceLimit, 'Keep the input unchanged and retry.');
    }
    await snapshotHandle.close();
    snapshotHandle = undefined;
    return {
      path: snapshotPath,
      dir: snapshotDir,
      handle: sourceHandle,
      identity: statIdentity(pathStat),
      bytes,
      sha256: hash.digest('hex'),
    };
  } catch (error) {
    if (snapshotHandle) await snapshotHandle.close().catch(() => {});
    if (sourceHandle) await sourceHandle.close().catch(() => {});
    if (snapshotDir) await fsp.rm(snapshotDir, { recursive: true, force: true }).catch(() => {});
    if (signal && signal.aborted) throw abortError('preflight', error);
    if (error && error.errorCode) throw error;
    throw structuredError('Input snapshot could not be created', 'E101', ERROR_CATEGORY.ResourceLimit, 'Verify input permissions and availability.', error);
  }
}

async function cleanupSourceSnapshot(snapshot) {
  if (!snapshot) return;
  let cleanupError;
  if (snapshot.handle) {
    try {
      await snapshot.handle.close();
    } catch (error) {
      cleanupError = error;
    }
  }
  try {
    await fsp.rm(snapshot.dir, { recursive: true, force: true });
  } catch (error) {
    cleanupError = cleanupError || error;
  }
  if (cleanupError) {
    throw structuredError('Private input snapshot could not be removed', 'E301', ERROR_CATEGORY.ResourceLimit, 'Verify temporary-directory permissions and retry.', cleanupError);
  }
}

function normalizeInputFormat(format) {
  if (format === 'jpg') return 'jpeg';
  if (format === 'jpeg' || format === 'png' || format === 'webp') return format;
  throw structuredError('Input format is unsupported or could not be determined', 'E111', ERROR_CATEGORY.CodecError, 'Use a static JPEG, PNG, or WebP image.');
}

function sourceFacts(metadata, digest) {
  if (!metadata || typeof metadata !== 'object') {
    throw structuredError('Input metadata could not be determined', 'E130', ERROR_CATEGORY.CodecError, 'Provide a decodable static image.');
  }
  if (!Number.isInteger(metadata.width) || !Number.isInteger(metadata.height)
    || metadata.width < 1 || metadata.height < 1
    || typeof metadata.hasAlpha !== 'boolean'
    || typeof metadata.isAnimated !== 'boolean') {
    throw structuredError('Input metadata is incomplete', 'E130', ERROR_CATEGORY.CodecError, 'Provide a decodable static image.');
  }
  const orientation = metadata.orientation === undefined ? null : metadata.orientation;
  if (orientation !== null && (!Number.isInteger(orientation) || orientation < 1 || orientation > 8)) {
    throw structuredError('Input orientation is invalid', 'E130', ERROR_CATEGORY.CodecError, 'Provide a decodable image with valid orientation metadata.');
  }
  return {
    sha256: digest.sha256,
    bytes: digest.bytes,
    format: normalizeInputFormat(metadata.format),
    width: metadata.width,
    height: metadata.height,
    hasAlpha: metadata.hasAlpha,
    isAnimated: metadata.isAnimated,
    orientationKnown: true,
    orientation,
  };
}

function compilerFingerprint(nativeVersion, nativeIdentity = {}) {
  const identity = {
    schemaVersion: 1,
    package: PACKAGE_NAME,
    packageVersion: PACKAGE_VERSION,
    nativeVersion,
    nativeIdentity,
    platform: process.platform,
    arch: process.arch,
  };
  return crypto.createHash('sha256').update(JSON.stringify(identity)).digest('hex');
}

async function resolveCommitPaths(outputDir) {
  let resolved;
  try {
    resolved = path.resolve(outputDir);
  } catch (error) {
    throw structuredError('Output directory path is invalid', 'E400', ERROR_CATEGORY.UserError, 'Provide a valid output directory path.', error);
  }
  const name = path.basename(resolved);
  if (!name || name === '.' || name === path.sep) {
    throw structuredError('Output directory must name a child directory', 'E400', ERROR_CATEGORY.UserError, 'Provide a versioned output directory path.');
  }

  let parent;
  try {
    parent = await fsp.realpath(path.dirname(resolved));
  } catch (error) {
    throw structuredError('Output parent directory is unavailable', 'E301', ERROR_CATEGORY.ResourceLimit, 'Create a trusted output parent and retry.', error);
  }
  const finalPath = path.join(parent, name);
  await ensureFinalAbsent(finalPath);
  return { parent, finalPath, name };
}

async function ensureFinalAbsent(finalPath) {
  try {
    await fsp.lstat(finalPath);
  } catch (error) {
    if (error && error.code === 'ENOENT') return;
    throw structuredError('Output path could not be checked', 'E301', ERROR_CATEGORY.ResourceLimit, 'Use an available output directory and retry.', error);
  }
  throw structuredError('Output directory already exists', 'E301', ERROR_CATEGORY.ResourceLimit, 'Publish to a new output directory; overwrite is not supported.');
}

function stagingPath(staging, relativePath) {
  if (typeof relativePath !== 'string' || relativePath.length === 0
    || path.posix.isAbsolute(relativePath)
    || relativePath.includes('\\')
    || path.posix.normalize(relativePath) !== relativePath) {
    throw structuredError('Compiler produced an unsafe relative output path', 'E900', ERROR_CATEGORY.InternalBug, 'Report the compiler path invariant failure.');
  }
  const resolved = path.resolve(staging, relativePath);
  const relative = path.relative(staging, resolved);
  if (!relative || relative.startsWith('..') || path.isAbsolute(relative)) {
    throw structuredError('Compiler produced an output path outside staging', 'E900', ERROR_CATEGORY.InternalBug, 'Report the compiler path invariant failure.');
  }
  return resolved;
}

async function createStaging(parent, name) {
  const staging = await fsp.mkdtemp(path.join(parent, `.${name}.lazy-image-staging-`));
  try {
    await fsp.chmod(staging, 0o700);
    await fsp.writeFile(path.join(staging, STAGING_MARKER), 'lazy-image staging v1\n', { flag: 'wx', mode: 0o600 });
  } catch (error) {
    await fsp.rm(staging, { recursive: true, force: true }).catch(() => {});
    throw structuredError('Private staging directory could not be prepared', 'E301', ERROR_CATEGORY.ResourceLimit, 'Verify output directory permissions and retry.', error);
  }
  return staging;
}

async function verifyStagingAllowlist(staging, plan) {
  const expected = new Set([STAGING_MARKER, 'manifest.json']);
  for (const artifact of plan.artifacts) expected.add(artifact.path);
  if (plan.placeholder) expected.add(plan.placeholder.path);
  let entries;
  try {
    entries = await fsp.readdir(staging);
  } catch (error) {
    throw structuredError('Private staging contents could not be listed', 'E900', ERROR_CATEGORY.InternalBug, 'Report the staging verification failure.', error);
  }
  if (entries.length !== expected.size || entries.some((entry) => !expected.has(entry))) {
    throw structuredError('Private staging contains unexpected files', 'E900', ERROR_CATEGORY.InternalBug, 'Report the staging verification failure.');
  }
  for (const entry of entries) {
    const entryPath = stagingPath(staging, entry);
    const stat = await fsp.lstat(entryPath);
    if (!stat.isFile() || stat.isSymbolicLink()) {
      throw structuredError('Private staging contains a non-regular file', 'E900', ERROR_CATEGORY.InternalBug, 'Report the staging verification failure.');
    }
  }
  const marker = await fsp.readFile(stagingPath(staging, STAGING_MARKER), 'utf8');
  if (marker !== 'lazy-image staging v1\n') {
    throw structuredError('Private staging marker is invalid', 'E900', ERROR_CATEGORY.InternalBug, 'Report the staging verification failure.');
  }
}

async function cleanupStaging(staging, plan, markerRemoved = false) {
  const stat = await fsp.lstat(staging);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error('staging path is no longer the owned directory');
  }
  const entries = await fsp.readdir(staging);
  if (entries.includes(STAGING_MARKER)) {
    const markerStat = await fsp.lstat(stagingPath(staging, STAGING_MARKER));
    if (!markerStat.isFile() || markerStat.isSymbolicLink()) throw new Error('staging marker is not a regular file');
    const marker = await fsp.readFile(stagingPath(staging, STAGING_MARKER), 'utf8');
    if (marker !== 'lazy-image staging v1\n') throw new Error('staging marker is invalid');
  } else if (!markerRemoved) {
    throw new Error('staging marker is missing');
  }
  const allowed = new Set([STAGING_MARKER, 'manifest.json']);
  if (plan) {
    for (const artifact of plan.artifacts) allowed.add(artifact.path);
    if (plan.placeholder) allowed.add(plan.placeholder.path);
  }
  for (const entry of entries) {
    if (!allowed.has(entry) && !/^\.manifest-[0-9a-f-]+\.tmp$/.test(entry)) {
      throw new Error('staging contains an unexpected entry');
    }
    const entryStat = await fsp.lstat(stagingPath(staging, entry));
    if (!entryStat.isFile() || entryStat.isSymbolicLink()) throw new Error('staging contains a non-regular entry');
  }
  await fsp.rm(staging, { recursive: true, force: true });
}

async function removeStagingMarker(staging, signal) {
  const markerPath = stagingPath(staging, STAGING_MARKER);
  const markerStat = await fsp.lstat(markerPath);
  if (!markerStat.isFile() || markerStat.isSymbolicLink()) throw new Error('staging marker is not a regular file');
  const marker = await fsp.readFile(markerPath, 'utf8');
  if (marker !== 'lazy-image staging v1\n') throw new Error('staging marker is invalid');
  throwIfAborted(signal, 'commit');
  await fsp.unlink(markerPath);
}

function outputFormat(format) {
  return format === 'jpg' ? 'jpeg' : format;
}

function privacyFailure() {
  return structuredError('Output metadata privacy invariant failed', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output metadata verification failure.');
}

class ContainerReader {
  constructor(handle, size, signal, phase) {
    this.handle = handle;
    this.size = size;
    this.signal = signal;
    this.phase = phase;
    this.position = 0;
    this.scratch = Buffer.allocUnsafe(64 * 1024);
  }

  async read(length) {
    if (!Number.isSafeInteger(length) || length < 0 || length > MAX_METADATA_BUFFER) {
      throw new Error('metadata field exceeds the bounded parser buffer');
    }
    const output = Buffer.allocUnsafe(length);
    let offset = 0;
    while (offset < length) {
      throwIfAborted(this.signal, this.phase);
      const { bytesRead } = await this.handle.read(output, offset, length - offset, this.position);
      if (bytesRead === 0) throw new Error('unexpected end of image container');
      offset += bytesRead;
      this.position += bytesRead;
    }
    return output;
  }

  async skip(length) {
    if (!Number.isSafeInteger(length) || length < 0 || this.position + length > this.size) {
      throw new Error('image container length is invalid');
    }
    let remaining = length;
    while (remaining > 0) {
      throwIfAborted(this.signal, this.phase);
      const amount = Math.min(remaining, this.scratch.length);
      const { bytesRead } = await this.handle.read(this.scratch, 0, amount, this.position);
      if (bytesRead === 0) throw new Error('unexpected end of image container');
      remaining -= bytesRead;
      this.position += bytesRead;
    }
  }

  async byte() {
    return (await this.read(1))[0];
  }

  async u16be() {
    return (await this.read(2)).readUInt16BE(0);
  }

  async u32be() {
    return (await this.read(4)).readUInt32BE(0);
  }

  async u32le() {
    return (await this.read(4)).readUInt32LE(0);
  }
}

async function readContainer(filePath, signal, phase, parser) {
  throwIfAborted(signal, phase);
  let before;
  try {
    before = await fsp.lstat(filePath);
  } catch (error) {
    throw structuredError('File could not be read for verification', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.', error);
  }
  if (!before.isFile() || before.isSymbolicLink()) {
    throw structuredError('File is not a regular file for verification', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  let handle;
  try {
    handle = await fsp.open(filePath, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
    const opened = await handle.stat();
    if (!opened.isFile() || !sameIdentity(statIdentity(before), statIdentity(opened))) {
      throw structuredError('File changed before verification', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
    }
    const result = await parser(new ContainerReader(handle, before.size, signal, phase), before.size);
    if (!result || result.complete !== true) {
      throw structuredError('Image container could not be completely verified', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
    }
    const after = await handle.stat();
    if (!sameIdentity(statIdentity(before), statIdentity(after))) {
      throw structuredError('File changed during verification', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
    }
    return result;
  } catch (error) {
    if (signal && signal.aborted) throw abortError(phase, error);
    if (error && error.errorCode) throw error;
    throw structuredError('Image container could not be verified', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.', error);
  } finally {
    if (handle) await handle.close().catch(() => {});
  }
}

const JPEG_ICC_PREFIX = Buffer.from('ICC_PROFILE\0');
const EXIF_PREFIX = Buffer.from('Exif\0\0');
const JFIF_CANONICAL = Buffer.from('4a46494600010100000100010000', 'hex');

function canonicalJfifPayload(payload) {
  return payload.equals(JFIF_CANONICAL);
}

function printableIccField(field) {
  return [...field].every((byte) => byte === 0 || (byte >= 32 && byte <= 126));
}

function validIccProfile(profile) {
  if (!Buffer.isBuffer(profile) || profile.length < 132 || profile.length % 4 !== 0) return false;
  if (profile.readUInt32BE(0) !== profile.length) return false;
  if (!printableIccField(profile.subarray(4, 8))) return false;
  if (!['mntr', 'scnr', 'prtr'].includes(profile.toString('ascii', 12, 16))) return false;
  if (!['RGB ', 'GRAY'].includes(profile.toString('ascii', 16, 20))) return false;
  if (!['XYZ ', 'Lab '].includes(profile.toString('ascii', 20, 24))) return false;
  if (profile.toString('ascii', 36, 40) !== 'acsp') return false;
  if (profile.subarray(100, 128).some((byte) => byte !== 0)) return false;
  if (![2, 4, 5].includes(profile[8])) return false;

  const tagCount = profile.readUInt32BE(128);
  if (tagCount > 4096) return false;
  const tableEnd = 132 + tagCount * 12;
  if (tableEnd > profile.length) return false;
  const signatures = new Set();
  const ranges = [];
  for (let offset = 132; offset < tableEnd; offset += 12) {
    const signature = profile.subarray(offset, offset + 4).toString('ascii');
    if (!printableIccField(profile.subarray(offset, offset + 4)) || signatures.has(signature)) return false;
    signatures.add(signature);
    const dataOffset = profile.readUInt32BE(offset + 4);
    const dataSize = profile.readUInt32BE(offset + 8);
    const dataEnd = dataOffset + dataSize;
    if (dataOffset < tableEnd || dataOffset % 4 !== 0 || dataSize < 8 || dataEnd > profile.length) return false;
    if (profile.subarray(dataOffset + 4, dataOffset + 8).some((byte) => byte !== 0)) return false;
    ranges.push([dataOffset, dataEnd]);
  }
  ranges.sort((left, right) => left[0] - right[0]);
  for (let index = 1; index < ranges.length; index += 1) {
    const previous = ranges[index - 1];
    const current = ranges[index];
    const sharedRange = previous[0] === current[0] && previous[1] === current[1];
    if (!sharedRange && previous[1] > current[0]) return false;
  }
  return true;
}

function recordIccProfile(state, profile, rejectPrivacy) {
  if (!Buffer.isBuffer(profile) || profile.length > MAX_ICC_BYTES || !validIccProfile(profile)) {
    if (rejectPrivacy) throw privacyFailure();
    state.iccOutcome = 'unsupported';
    return;
  }
  if (state.iccOutcome === 'preserved') {
    if (rejectPrivacy) throw privacyFailure();
    state.iccOutcome = 'unsupported';
    return;
  }
  state.iccOutcome = 'preserved';
}

function finalizeJpegIcc(state, rejectPrivacy) {
  const partCount = state.iccParts.reduce((count, part) => count + (part ? 1 : 0), 0);
  if (partCount === 0) return state.iccOutcome || 'absent';
  if (state.iccCount === null || partCount !== state.iccCount) {
    if (rejectPrivacy) throw privacyFailure();
    state.iccOutcome = 'unsupported';
    return state.iccOutcome;
  }
  const ordered = [];
  for (let sequence = 1; sequence <= state.iccCount; sequence += 1) {
    const part = state.iccParts[sequence];
    if (!part) {
      if (rejectPrivacy) throw privacyFailure();
      state.iccOutcome = 'unsupported';
      return state.iccOutcome;
    }
    ordered.push(part);
  }
  const profile = Buffer.concat(ordered);
  recordIccProfile(state, profile, rejectPrivacy);
  return state.iccOutcome;
}

async function readJpegMarker(reader) {
  if (reader.position >= reader.size || await reader.byte() !== 0xff) return null;
  let marker = await reader.byte();
  while (marker === 0xff) {
    if (reader.position >= reader.size) return null;
    marker = await reader.byte();
  }
  return marker;
}

// JPEG entropy data uses FF00 stuffing and restart markers; return the next
// real marker after consuming the complete scan, without buffering the image.
async function consumeJpegScan(reader) {
  while (reader.position < reader.size) {
    if (await reader.byte() !== 0xff) continue;
    if (reader.position >= reader.size) return null;
    let marker = await reader.byte();
    while (marker === 0xff) {
      if (reader.position >= reader.size) return null;
      marker = await reader.byte();
    }
    if (marker === 0x00 || (marker >= 0xd0 && marker <= 0xd7)) continue;
    return marker;
  }
  return null;
}

async function parseJpegContainer(reader, rejectPrivacy) {
  const soi = await reader.read(2);
  if (soi[0] !== 0xff || soi[1] !== 0xd8) return { complete: false };
  let hasExif = false;
  const state = { iccParts: [], iccCount: null, iccOutcome: 'absent' };
  let pendingMarker;
  while (reader.position < reader.size || pendingMarker !== undefined) {
    const marker = pendingMarker === undefined ? await readJpegMarker(reader) : pendingMarker;
    pendingMarker = undefined;
    if (marker === null) return { complete: false };
    if (marker === 0xd9) {
      if (reader.position !== reader.size) return { complete: false };
      return { complete: true, hasExif, iccOutcome: finalizeJpegIcc(state, rejectPrivacy) };
    }
    if (marker === 0xda) {
      if (reader.position + 2 > reader.size) return { complete: false };
      const scanLength = await reader.u16be();
      if (scanLength < 2 || scanLength - 2 > reader.size - reader.position) return { complete: false };
      await reader.skip(scanLength - 2);
      const nextMarker = await consumeJpegScan(reader);
      if (nextMarker === null) return { complete: false };
      pendingMarker = nextMarker;
      continue;
    }
    if (marker === 0x01 || (marker >= 0xd0 && marker <= 0xd7)) continue;
    if (marker === 0xd8) return { complete: false };
    if (reader.position + 2 > reader.size) return { complete: false };
    const length = await reader.u16be();
    if (length < 2 || length - 2 > reader.size - reader.position) return { complete: false };
    const payloadLength = length - 2;
    if (marker === 0xfe || (marker >= 0xe0 && marker <= 0xef)) {
      const payload = await reader.read(payloadLength);
      const isExif = marker === 0xe1 && payload.subarray(0, EXIF_PREFIX.length).equals(EXIF_PREFIX);
      const isIcc = marker === 0xe2 && payload.subarray(0, JPEG_ICC_PREFIX.length).equals(JPEG_ICC_PREFIX);
      const isJfif = marker === 0xe0 && canonicalJfifPayload(payload);
      hasExif ||= isExif;
      if (isIcc) {
        if (payload.length < 14) {
          if (rejectPrivacy) throw privacyFailure();
          state.iccOutcome = 'unsupported';
        } else {
          const sequence = payload[12];
          const count = payload[13];
          if (sequence === 0 || count === 0 || sequence > count || (state.iccCount !== null && state.iccCount !== count)
            || state.iccParts[sequence]) {
            if (rejectPrivacy) throw privacyFailure();
            state.iccOutcome = 'unsupported';
          } else {
            state.iccCount = count;
            state.iccParts[sequence] = payload.subarray(14);
            if (state.iccParts.reduce((total, part) => total + (part ? part.length : 0), 0) > MAX_ICC_BYTES) {
              throw privacyFailure();
            }
          }
        }
      }
      if (rejectPrivacy && (marker === 0xfe || (marker >= 0xe0 && marker <= 0xef
        && !isIcc && !isJfif))) {
        throw privacyFailure();
      }
    } else {
      await reader.skip(payloadLength);
    }
  }
  return { complete: false };
}

const PNG_SIGNATURE = Buffer.from('\x89PNG\r\n\x1a\n', 'binary');
const PNG_ALLOWED = new Set(['IHDR', 'PLTE', 'IDAT', 'IEND', 'iCCP', 'sRGB', 'gAMA', 'cHRM', 'tRNS']);

async function parsePngContainer(reader, rejectPrivacy) {
  if (!(await reader.read(8)).equals(PNG_SIGNATURE)) return { complete: false };
  let hasExif = false;
  const state = { iccOutcome: 'absent' };
  while (reader.position < reader.size) {
    const length = await reader.u32be();
    const type = (await reader.read(4)).toString('ascii');
    if (length > reader.size - reader.position - 4) return { complete: false };
    if (type === 'eXIf') hasExif = true;
    if (rejectPrivacy && !PNG_ALLOWED.has(type)) throw privacyFailure();
    if (type === 'iCCP') {
      if (length > MAX_ICC_BYTES + MAX_METADATA_BUFFER) throw privacyFailure();
      const payload = await reader.read(length);
      const nameEnd = payload.indexOf(0);
      if (nameEnd < 0 || nameEnd + 2 > payload.length || payload[nameEnd + 1] !== 0) {
        if (rejectPrivacy) throw privacyFailure();
        state.iccOutcome = 'unsupported';
      } else {
        try {
          const profile = zlib.inflateSync(payload.subarray(nameEnd + 2), { maxOutputLength: MAX_ICC_BYTES });
          recordIccProfile(state, profile, rejectPrivacy);
        } catch (error) {
          if (rejectPrivacy) throw privacyFailure();
          state.iccOutcome = 'unsupported';
        }
      }
    } else {
      await reader.skip(length);
    }
    await reader.skip(4);
    if (type === 'IEND') {
      if (length !== 0) return { complete: false };
      return { complete: reader.position === reader.size, hasExif, iccOutcome: state.iccOutcome };
    }
  }
  return { complete: false };
}

async function parseWebpContainer(reader, rejectPrivacy) {
  if ((await reader.read(4)).toString('ascii') !== 'RIFF') return { complete: false };
  const riffSize = await reader.u32le();
  if ((await reader.read(4)).toString('ascii') !== 'WEBP' || riffSize + 8 !== reader.size) return { complete: false };
  const allowed = new Set(['VP8 ', 'VP8L', 'VP8X', 'ALPH', 'ICCP']);
  let hasExif = false;
  const state = { iccOutcome: 'absent' };
  while (reader.position < reader.size) {
    const type = (await reader.read(4)).toString('ascii');
    const length = await reader.u32le();
    if (length > reader.size - reader.position) return { complete: false };
    if (type === 'EXIF') hasExif = true;
    if (rejectPrivacy && (!allowed.has(type) || type === 'EXIF' || type === 'XMP ')) {
      throw privacyFailure();
    }
    if (type === 'VP8X' && length > 0) {
      const flags = await reader.byte();
      if ((flags & 0x02) !== 0) throw privacyFailure();
      await reader.skip(length - 1);
    } else if (type === 'ICCP') {
      if (length > MAX_ICC_BYTES) throw privacyFailure();
      const profile = await reader.read(length);
      recordIccProfile(state, profile, rejectPrivacy);
    } else {
      await reader.skip(length);
    }
    if ((length & 1) !== 0) await reader.skip(1);
  }
  return { complete: reader.position === reader.size, hasExif, iccOutcome: state.iccOutcome };
}

const AVIF_ALLOWED = {
  top: new Set(['ftyp', 'meta', 'mdat']),
  meta: new Set(['hdlr', 'pitm', 'iloc', 'iinf', 'iprp']),
  iinf: new Set(['infe']),
  iprp: new Set(['ipco', 'ipma']),
  ipco: new Set(['ispe', 'pixi', 'av1C', 'colr']),
};
const AVIF_FORBIDDEN = new Set(['Exif', 'xml ', 'XMP ', 'mime', 'uuid']);
const AVIF_ITEM_FORBIDDEN = [Buffer.from('Exif'), Buffer.from('mime'), Buffer.from('xml '), Buffer.from('XMP ')];
const AVIF_FTYP_PAYLOAD = Buffer.from('6176696600000000617669666d6966316d6961664d413142', 'hex');
const AVIF_HANDLER = Buffer.from('0000000000000000706963740000000000000000000000006c69626176696600', 'hex');
const AVIF_ITEM = Buffer.from('020000000001000061763031436f6c6f7200', 'hex');
const AVIF_IPMA_BASE_ASSOCIATIONS = Buffer.from([1, 2, 0x83, 4]);

async function readAvifBox(reader, fileSize) {
  const start = reader.position;
  if (start + 8 > fileSize) return null;
  const size32 = await reader.u32be();
  const type = (await reader.read(4)).toString('ascii');
  let headerSize = 8;
  let boxSize = size32;
  if (size32 === 1) {
    const high = await reader.u32be();
    const low = await reader.u32be();
    if (high !== 0) return null;
    boxSize = low;
    headerSize = 16;
  } else if (size32 === 0) {
    boxSize = fileSize - start;
  }
  if (boxSize < headerSize || boxSize > fileSize - start) return null;
  return { start, type, headerSize, size: boxSize, end: start + boxSize, payload: boxSize - headerSize };
}

async function parseAvifBoxesStream(reader, fileSize, end, state, rejectPrivacy, context = 'top', depth = 0) {
  if (depth > 12) return false;
  while (reader.position < end) {
    const box = await readAvifBox(reader, fileSize);
    if (!box || box.end > end) return false;
    if (!AVIF_ALLOWED[context] || !AVIF_ALLOWED[context].has(box.type) || AVIF_FORBIDDEN.has(box.type)) {
      throw privacyFailure();
    }
    if (box.type === 'ftyp') {
      if (box.payload !== AVIF_FTYP_PAYLOAD.length) throw privacyFailure();
      const data = await reader.read(box.payload);
      if (!data.equals(AVIF_FTYP_PAYLOAD)) throw privacyFailure();
      state.ftyp = true;
    } else if (box.type === 'hdlr') {
      const data = await reader.read(box.payload);
      if (!data.equals(AVIF_HANDLER)) throw privacyFailure();
    } else if (box.type === 'pitm') {
      const data = await reader.read(box.payload);
      if (data.length !== 6 || data.readUInt32BE(0) !== 0 || data.readUInt16BE(4) !== 1) throw privacyFailure();
    } else if (box.type === 'iloc') {
      const data = await reader.read(box.payload);
      if (data.length !== 22 || data.readUInt32BE(0) !== 0 || data[4] !== 0x44 || data[5] !== 0
        || data.readUInt16BE(6) !== 1 || data.readUInt16BE(8) !== 1 || data.readUInt16BE(10) !== 0
        || data.readUInt16BE(12) !== 1) throw privacyFailure();
      state.extent = { offset: data.readUInt32BE(14), length: data.readUInt32BE(18) };
      if (state.extent.offset < 1 || state.extent.length < 1) throw privacyFailure();
    } else if (box.type === 'ispe') {
      if (box.payload !== 12) return false;
      const data = await reader.read(12);
      if (data.readUInt32BE(0) !== 0) throw privacyFailure();
      state.width = data.readUInt32BE(4);
      state.height = data.readUInt32BE(8);
    } else if (box.type === 'pixi') {
      const data = await reader.read(box.payload);
      if (data.length !== 8 || data.readUInt32BE(0) !== 0 || data[4] !== 3 || data[5] !== 8
        || data[6] !== 8 || data[7] !== 8) throw privacyFailure();
    } else if (box.type === 'av1C') {
      const data = await reader.read(box.payload);
      if (!data.equals(Buffer.from([0x81, 0x1f, 0x0c, 0x00]))) throw privacyFailure();
    } else if (box.type === 'colr') {
      if (box.payload < 4) return false;
      const colourType = (await reader.read(4)).toString('ascii');
      if (colourType === 'prof') {
        if (box.payload - 4 > MAX_ICC_BYTES) throw privacyFailure();
        recordIccProfile(state, await reader.read(box.payload - 4), rejectPrivacy);
      } else if (colourType === 'nclx') {
        const data = await reader.read(box.payload - 4);
        if (!data.equals(Buffer.from([0x00, 0x01, 0x00, 0x0d, 0x00, 0x01, 0x80]))) throw privacyFailure();
      } else {
        throw privacyFailure();
      }
    } else if (box.type === 'infe') {
      const data = await reader.read(box.payload);
      if (!data.equals(AVIF_ITEM) || AVIF_ITEM_FORBIDDEN.some((marker) => data.includes(marker))) throw privacyFailure();
    } else if (box.type === 'ipma') {
      const data = await reader.read(box.payload);
      const associations = state.iccOutcome === 'preserved'
        ? Buffer.concat([AVIF_IPMA_BASE_ASSOCIATIONS, Buffer.from([5])])
        : AVIF_IPMA_BASE_ASSOCIATIONS;
      if (data.length !== 11 + associations.length || data.readUInt32BE(0) !== 0 || data.readUInt32BE(4) !== 1
        || data.readUInt16BE(8) !== 1 || data[10] !== associations.length
        || !data.subarray(11).equals(associations)) throw privacyFailure();
    } else if (box.type === 'mdat') {
      if (state.mdat) throw privacyFailure();
      state.mdat = { offset: reader.position, length: box.payload };
      await reader.skip(box.payload);
    } else if (box.type === 'meta' || box.type === 'iinf' || box.type === 'iprp' || box.type === 'ipco') {
      if (box.type === 'meta') {
        const fullbox = await reader.read(Math.min(4, box.payload));
        if (fullbox.length !== 4 || fullbox.readUInt32BE(0) !== 0) return false;
      } else if (box.type === 'iinf') {
        if (box.payload < 6) return false;
        const fullbox = await reader.read(4);
        if (fullbox.readUInt32BE(0) !== 0 || (await reader.u16be()) !== 1) return false;
      }
      const childContext = box.type;
      if (!await parseAvifBoxesStream(reader, fileSize, box.end, state, rejectPrivacy, childContext, depth + 1)) return false;
    }
    if (reader.position !== box.end) return false;
  }
  return reader.position === end;
}

async function parseAvifContainer(reader, rejectPrivacy) {
  const state = { iccOutcome: 'absent' };
  const complete = await parseAvifBoxesStream(reader, reader.size, reader.size, state, rejectPrivacy);
  const extentMatchesData = state.extent && state.mdat
    && state.extent.offset === state.mdat.offset
    && state.extent.length === state.mdat.length;
  return { complete: complete && Boolean(state.ftyp && extentMatchesData), ...state, isAnimated: false };
}

async function inspectAvifFile(filePath, signal) {
  const result = await readContainer(filePath, signal, 'verification', (reader) => parseAvifContainer(reader, true));
  if (!result.ftyp || !Number.isInteger(result.width) || !Number.isInteger(result.height)
    || result.width < 1 || result.height < 1) {
    throw structuredError('AVIF output could not be structurally verified', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  return { format: 'avif', width: result.width, height: result.height, isAnimated: result.isAnimated, iccOutcome: result.iccOutcome };
}

async function verifyPrivacy(filePath, format, signal) {
  const result = await readContainer(filePath, signal, 'verification', (reader) => {
    if (format === 'jpeg') return parseJpegContainer(reader, true);
    if (format === 'png') return parsePngContainer(reader, true);
    if (format === 'webp') return parseWebpContainer(reader, true);
    return parseAvifContainer(reader, true);
  });
  return result.iccOutcome || 'absent';
}

async function rejectUnknownOrientation(filePath, metadata, signal) {
  if (metadata.orientation !== undefined) return;
  let result;
  try {
    result = await readContainer(filePath, signal, 'preflight', (reader) => {
      if (metadata.format === 'jpeg') return parseJpegContainer(reader, false);
      if (metadata.format === 'png') return parsePngContainer(reader, false);
      if (metadata.format === 'webp') return parseWebpContainer(reader, false);
      return { complete: false };
    });
  } catch (error) {
    if (isAbortError(error)) throw error;
    throw structuredError('Input orientation could not be determined', 'E130', ERROR_CATEGORY.CodecError, 'Remove or normalize EXIF orientation before compiling the upload.', error);
  }
  if (result.hasExif) {
    throw structuredError('Input EXIF orientation could not be determined within the safe inspection contract', 'E130', ERROR_CATEGORY.CodecError, 'Remove or normalize EXIF orientation before compiling the upload.');
  }
}

function verifyIccOutcome(metrics) {
  if (!metrics || typeof metrics.iccOutcome !== 'string' || !ICC_OUTCOMES.has(metrics.iccOutcome)) {
    throw structuredError('Output ICC outcome is unavailable', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output metrics invariant failure.');
  }
  return metrics.iccOutcome;
}

async function verifyArtifact({ artifact, outputPath, result, api, signal }) {
  throwIfAborted(signal, 'verification');
  let before;
  try {
    before = await fsp.lstat(outputPath);
  } catch (error) {
    throw structuredError('Generated artifact is missing', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.', error);
  }
  if (!before.isFile() || before.isSymbolicLink()) {
    throw structuredError('Generated artifact is not a regular file', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }

  let metadata;
  try {
    metadata = artifact.format === 'avif'
      ? await inspectAvifFile(outputPath, signal)
      : api.inspectFile(outputPath);
  } catch (error) {
    throw error;
  }
  if (outputFormat(metadata.format) !== artifact.format || metadata.isAnimated === true || metadata.width !== artifact.width) {
    throw structuredError('Generated artifact does not match the execution plan', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  if (!Number.isInteger(metadata.height) || metadata.height < 1) {
    throw structuredError('Generated artifact dimensions are invalid', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }

  const bytes = before.size;
  if (!Number.isInteger(bytes) || bytes < 1 || result.bytesWritten !== bytes) {
    throw structuredError('Generated artifact byte count is inconsistent', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  if (result.metrics && result.metrics.bytesOut !== undefined && result.metrics.bytesOut !== bytes) {
    throw structuredError('Encoder and filesystem byte counts differ', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  if (result.metrics && result.metrics.formatOut && outputFormat(result.metrics.formatOut) !== artifact.format) {
    throw structuredError('Encoder format differs from the execution plan', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  if (artifact.targetBytes !== undefined && (result.budgetMet !== true || bytes > artifact.targetBytes)) {
    throw structuredError('Strict byte budget was not met', 'E300', ERROR_CATEGORY.CodecError, 'Use a larger targetBytes budget or another output format.');
  }

  const iccOutcome = verifyIccOutcome(result.metrics);
  const observedIccOutcome = await verifyPrivacy(outputPath, artifact.format, signal);
  if ((iccOutcome === 'preserved') !== (observedIccOutcome === 'preserved')) {
    throw structuredError('Output ICC outcome does not match its container', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output metadata verification failure.');
  }
  const digest = await hashRegularFile(outputPath, signal, 'verification');
  const after = await fsp.lstat(outputPath);
  if (!after.isFile() || !sameIdentity(statIdentity(before), statIdentity(after))) {
    throw structuredError('Generated artifact changed during verification', 'E900', ERROR_CATEGORY.InternalBug, 'Report the output verification failure.');
  }
  return {
    id: artifact.id,
    path: artifact.path,
    format: artifact.format,
    width: artifact.width,
    height: metadata.height,
    bytes,
    quality: artifact.targetBytes === undefined ? artifact.quality : result.quality,
    ...(artifact.targetBytes === undefined ? {} : { targetBytes: artifact.targetBytes }),
    iccOutcome,
    sha256: digest.sha256,
  };
}

async function readBoundedDataUrl(filePath, expectedSha256, signal) {
  throwIfAborted(signal, 'verification');
  const stat = await fsp.lstat(filePath);
  if (!stat.isFile() || stat.isSymbolicLink() || stat.size > PLACEHOLDER_MAX_BYTES) {
    throw structuredError('Placeholder exceeds the bounded read limit', 'E900', ERROR_CATEGORY.InternalBug, 'Report the placeholder verification failure.');
  }
  let handle;
  try {
    handle = await fsp.open(filePath, fs.constants.O_RDONLY | (fs.constants.O_NOFOLLOW || 0));
    const opened = await handle.stat();
    if (!opened.isFile() || !sameIdentity(statIdentity(stat), statIdentity(opened))) {
      throw structuredError('Placeholder changed before bounded read', 'E900', ERROR_CATEGORY.InternalBug, 'Report the placeholder verification failure.');
    }
    const data = Buffer.allocUnsafe(stat.size);
    let offset = 0;
    while (offset < data.length) {
      throwIfAborted(signal, 'verification');
      const { bytesRead } = await handle.read(data, offset, data.length - offset, offset);
      if (bytesRead === 0) break;
      offset += bytesRead;
    }
    if (offset !== data.length) {
      throw structuredError('Placeholder changed during bounded read', 'E900', ERROR_CATEGORY.InternalBug, 'Report the placeholder verification failure.');
    }
    const after = await handle.stat();
    if (!sameIdentity(statIdentity(stat), statIdentity(after))) {
      throw structuredError('Placeholder changed during bounded read', 'E900', ERROR_CATEGORY.InternalBug, 'Report the placeholder verification failure.');
    }
    const digest = crypto.createHash('sha256').update(data).digest('hex');
    if (digest !== expectedSha256) {
      throw structuredError('Placeholder hash changed during bounded read', 'E900', ERROR_CATEGORY.InternalBug, 'Report the placeholder verification failure.');
    }
    return `data:image/webp;base64,${data.toString('base64')}`;
  } finally {
    if (handle) await handle.close().catch(() => {});
  }
}

async function verifySourceUnchanged(snapshot, signal) {
  throwIfAborted(signal, 'verification');
  const stat = await snapshot.handle.stat();
  if (!stat.isFile() || !sameIdentity(snapshot.identity, statIdentity(stat))) {
    throw structuredError('Input changed during compilation', 'E101', ERROR_CATEGORY.ResourceLimit, 'Keep the input unchanged and retry.');
  }
  const digest = await hashHandle(snapshot.handle, signal, MAX_INPUT_BYTES, 'verification');
  const after = await snapshot.handle.stat();
  if (!sameIdentity(snapshot.identity, statIdentity(after))) {
    throw structuredError('Input changed during compilation', 'E101', ERROR_CATEGORY.ResourceLimit, 'Keep the input unchanged and retry.');
  }
  if (digest.sha256 !== snapshot.sha256 || digest.bytes !== snapshot.bytes) {
    throw structuredError('Input content changed during compilation', 'E101', ERROR_CATEGORY.ResourceLimit, 'Keep the input unchanged and retry.');
  }
}

function manifestFor(plan, compiler, artifactResults, placeholder) {
  return {
    schemaVersion: 1,
    compiler: {
      name: PACKAGE_NAME,
      version: compiler.version,
      platform: process.platform,
      arch: process.arch,
      fingerprint: compiler.fingerprint,
    },
    policy: {
      preset: 'publicUpload',
      fingerprint: plan.policyFingerprint,
      widths: [...plan.policy.widths],
      formats: [...plan.policy.formats],
      placeholder: plan.policy.placeholder,
    },
    source: {
      sha256: plan.source.sha256,
      bytes: plan.source.bytes,
      detectedFormat: plan.source.detectedFormat,
      encodedWidth: plan.source.encodedWidth,
      encodedHeight: plan.source.encodedHeight,
      displayWidth: plan.source.displayWidth,
      displayHeight: plan.source.displayHeight,
      hasAlpha: plan.source.hasAlpha,
      orientation: plan.source.orientation,
      orientationApplied: plan.source.orientationApplied,
    },
    metadata: {
      mode: 'privacySafe',
      exif: 'stripped',
      gps: 'stripped',
      xmp: 'stripped',
      comments: 'stripped',
      unknownAncillary: 'stripped',
      artifactIccPolicy: 'preserve-if-safe',
      placeholderIccPolicy: 'strip',
    },
    artifacts: artifactResults,
    delivery: {
      srcsets: plan.delivery.srcsets.map(({ format, value }) => ({ format, value })),
    },
    ...(placeholder ? { placeholder } : {}),
    cacheKey: plan.cacheKey,
  };
}

async function compileImage(options) {
  let input;
  try {
    input = validateOptions(options);
  } catch (error) {
    throw wrapError(error, 'preflight', undefined, { errorCode: 'E400', category: ERROR_CATEGORY.UserError });
  }

  const { inputPath, outputDir, policy, signal } = input;
  let resolvedInput;
  try {
    resolvedInput = path.resolve(inputPath);
  } catch (error) {
    throw wrapError(error, 'preflight', undefined, { errorCode: 'E400', category: ERROR_CATEGORY.UserError });
  }

  let sourceSnapshot;
  let digest;
  let facts;
  let api;
  let compiler;
  let plan;
  let commitPaths;
  let failure;
  try {
    throwIfAborted(signal, 'preflight');
    sourceSnapshot = await createSourceSnapshot(resolvedInput, signal);
    digest = { bytes: sourceSnapshot.bytes, sha256: sourceSnapshot.sha256 };

    // Load the package lazily to avoid an index.js circular dependency during startup.
    api = require('../index');
    const metadata = api.inspectFile(sourceSnapshot.path);
    await rejectUnknownOrientation(sourceSnapshot.path, metadata, signal);
    facts = sourceFacts(metadata, digest);
    const nativeVersion = api.version();
    if (typeof nativeVersion !== 'string' || nativeVersion.length === 0) {
      throw structuredError('Native compiler version is unavailable', 'E900', ERROR_CATEGORY.InternalBug, 'Report the compiler identity failure.');
    }
    let supportedFormats;
    try {
      supportedFormats = api.supportedOutputFormats();
    } catch (error) {
      throw structuredError('Native compiler capabilities are unavailable', 'E900', ERROR_CATEGORY.InternalBug, 'Report the compiler identity failure.', error);
    }
    if (!Array.isArray(supportedFormats) || supportedFormats.some((format) => typeof format !== 'string')) {
      throw structuredError('Native compiler capabilities are invalid', 'E900', ERROR_CATEGORY.InternalBug, 'Report the compiler identity failure.');
    }
    const nativeBuildIdentity = typeof api.compilerIdentity === 'function' ? api.compilerIdentity() : undefined;
    if (typeof nativeBuildIdentity !== 'string' || nativeBuildIdentity.length === 0) {
      throw structuredError('Native compiler build identity is unavailable', 'E900', ERROR_CATEGORY.InternalBug, 'Report the compiler identity failure.');
    }
    const nativeIdentity = {
      build: nativeBuildIdentity,
      nodeAbi: process.versions.modules,
      outputFormats: [...new Set(supportedFormats.map((format) => String(format).toLowerCase()))].sort(),
    };
    compiler = { version: nativeVersion, fingerprint: compilerFingerprint(nativeVersion, nativeIdentity) };
    // Keep the existing public-upload firewall as the only native processing boundary.
    const baseEngine = api.ImageEngine.fromPath(sourceSnapshot.path).sanitize({ policy: 'public-upload' });
    try {
      plan = compileArtifactPlan(facts, policy, compiler.fingerprint);
    } catch (error) {
      throw wrapError(error, 'planning', undefined, { errorCode: 'E400', category: ERROR_CATEGORY.UserError });
    }
    try {
      commitPaths = await resolveCommitPaths(outputDir);
    } catch (error) {
      throw wrapError(error, 'commit', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
    }

    let staging;
    try {
      staging = await createStaging(commitPaths.parent, commitPaths.name);
    } catch (error) {
      throw wrapError(error, 'commit', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
    }

    let committed = false;
    let markerRemoved = false;
    let primaryError;
    try {
      const results = [];
      for (const artifact of plan.artifacts) {
        throwIfAborted(signal, 'processing');
        const outputPath = stagingPath(staging, artifact.path);
        let result;
        try {
          const engine = baseEngine.clone().resize({ width: artifact.width, fit: 'inside' });
          if (artifact.targetBytes === undefined) {
            result = await engine.toFileWithMetrics(outputPath, artifact.format, artifact.quality, false);
          } else {
            result = await engine.toFileTargetBytes(outputPath, artifact.format, {
              targetBytes: artifact.targetBytes,
              strict: true,
              qualityFloorPolicy: 'strict',
            });
          }
        } catch (error) {
          if (signal && signal.aborted) throw abortError('processing', error);
          throw wrapError(error, 'processing', artifact.id);
        }
        try {
          results.push(await verifyArtifact({ artifact, outputPath, result, api, signal }));
        } catch (error) {
          if (signal && signal.aborted) throw abortError('verification', error);
          throw wrapError(error, 'verification', artifact.id);
        }
      }

      let placeholder;
      if (plan.placeholder) {
        throwIfAborted(signal, 'processing');
        const placeholderPath = stagingPath(staging, plan.placeholder.path);
        let result;
        try {
          // The placeholder intentionally uses an explicit strip-all metadata policy;
          // public-upload artifacts retain safe ICC profiles.
          const placeholderEngine = api.ImageEngine.fromPath(sourceSnapshot.path)
            .keepMetadata({ icc: false, exif: false, xmp: false, stripGps: true })
            .limits({ maxPixels: MAX_PIXELS, maxBytes: MAX_INPUT_BYTES, timeoutMs: 5_000 })
            .resize({ width: plan.placeholder.width, fit: 'inside' });
          result = await placeholderEngine.toFileWithMetrics(placeholderPath, 'webp', plan.placeholder.quality, false);
        } catch (error) {
          if (signal && signal.aborted) throw abortError('processing', error);
          throw wrapError(error, 'processing', plan.placeholder.id);
        }
        let verified;
        try {
          verified = await verifyArtifact({ artifact: plan.placeholder, outputPath: placeholderPath, result, api, signal });
          const dataUrl = await readBoundedDataUrl(placeholderPath, verified.sha256, signal);
          placeholder = {
            path: plan.placeholder.path,
            dataUrl,
            format: 'webp',
            width: verified.width,
            height: verified.height,
            bytes: verified.bytes,
            iccOutcome: 'stripped-for-placeholder',
            sha256: verified.sha256,
          };
        } catch (error) {
          if (signal && signal.aborted) throw abortError('verification', error);
          throw wrapError(error, 'verification', plan.placeholder.id);
        }
      }

      await verifySourceUnchanged(sourceSnapshot, signal);
      try {
        await cleanupSourceSnapshot(sourceSnapshot);
      } catch (error) {
        throw wrapError(error, 'cleanup', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
      }
      sourceSnapshot = undefined;

      const manifest = manifestFor(plan, compiler, results, placeholder);
      const manifestPath = stagingPath(staging, 'manifest.json');
      const manifestTempPath = stagingPath(staging, `.manifest-${crypto.randomUUID()}.tmp`);
      const manifestBytes = JSON.stringify(manifest);
      try {
        await fsp.writeFile(manifestTempPath, manifestBytes, { flag: 'wx', mode: 0o600 });
        await fsp.rename(manifestTempPath, manifestPath);
      } catch (error) {
        throw wrapError(error, 'commit', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
      }

      try {
        await verifyStagingAllowlist(staging, plan);
        await ensureFinalAbsent(commitPaths.finalPath);
        throwIfAborted(signal, 'commit');
        await removeStagingMarker(staging, signal);
        markerRemoved = true;
        await fsp.rename(staging, commitPaths.finalPath);
        committed = true;
      } catch (error) {
        throw wrapError(error, 'commit', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
      }
      return manifest;
    } catch (error) {
      primaryError = error;
      throw error;
    } finally {
      if (!committed) {
        try {
          await cleanupStaging(staging, plan, markerRemoved);
        } catch (cleanupError) {
          if (primaryError) {
            primaryError.cleanupError = cleanupError;
          } else {
            throw wrapError(cleanupError, 'cleanup', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
          }
        }
      }
    }
  } catch (error) {
    let finalError;
    if (signal && signal.aborted) {
      finalError = isAbortError(error) ? error : abortError(error.phase || 'preflight', error);
    } else if (error instanceof ArtifactCompilationError) {
      finalError = error;
    } else {
      finalError = wrapError(error, error.phase || 'preflight');
    }
    failure = finalError;
    throw finalError;
  } finally {
    if (sourceSnapshot) {
      try {
        await cleanupSourceSnapshot(sourceSnapshot);
      } catch (cleanupError) {
        if (failure) {
          failure.cleanupError = cleanupError;
        } else {
          throw wrapError(cleanupError, 'cleanup', undefined, { errorCode: 'E301', category: ERROR_CATEGORY.ResourceLimit });
        }
      }
    }
  }
}

module.exports = { ArtifactCompilationError, compileImage, compilerFingerprint };
