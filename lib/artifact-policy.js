'use strict';

const { createHash } = require('node:crypto');

const MAX_INPUT_BYTES = 32 * 1024 * 1024;
const MAX_PIXELS = 40_000_000;
const MAX_DIMENSION = 32_768;
const MAX_WIDTHS = 8;
const MAX_FORMATS = 3;
const MAX_ARTIFACTS = 24;
const MAX_RAW_WIDTHS = 64;
const MAX_RAW_FORMATS = 12;
const MAX_BUDGET_BYTES = 32 * 1024 * 1024;
const FORMAT_ORDER = ['jpeg', 'webp', 'avif'];
const DEFAULT_WIDTHS = [320, 640, 1280];
const DEFAULT_QUALITY = { jpeg: 85, webp: 80, avif: 60 };
const POLICY_KEYS = new Set(['preset', 'widths', 'formats', 'budgets', 'placeholder']);
const SOURCE_KEYS = new Set([
  'sha256', 'bytes', 'format', 'width', 'height', 'hasAlpha', 'isAnimated',
  'orientationKnown', 'orientation',
]);
const BUDGET_KEYS = new Set(['width', 'format', 'maxBytes']);

function fail(message) {
  const error = new TypeError(`[E400] ${message}`);
  error.code = 'LAZY_IMAGE_USER_ERROR';
  error.errorCode = 'E400';
  error.errorCategory = 'UserError';
  error.recoveryHint = 'Provide validated source facts and a valid publicUpload policy.';
  throw error;
}

function requireRecord(value, name) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) {
    fail(`${name} must be an object`);
  }
  const prototype = Object.getPrototypeOf(value);
  if (prototype !== Object.prototype && prototype !== null) {
    fail(`${name} must be a plain object`);
  }
  return value;
}

function rejectUnknownKeys(value, allowed, name) {
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string' || !allowed.has(key)) fail(`unknown ${name} key: ${String(key)}`);
  }
}

function requireOwnData(value, key, name) {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  if (!descriptor || !Object.hasOwn(descriptor, 'value')) {
    fail(`${name}.${key} must be an own data property`);
  }
  return descriptor.value;
}

function optionalOwnData(value, key, name) {
  const descriptor = Object.getOwnPropertyDescriptor(value, key);
  if (!descriptor) {
    if (key in value) fail(`${name}.${key} must be an own data property`);
    return undefined;
  }
  if (!Object.hasOwn(descriptor, 'value')) {
    fail(`${name}.${key} must be an own data property`);
  }
  return descriptor.value;
}

function snapshotArray(value, name) {
  const snapshot = [];
  for (let index = 0; index < value.length; index += 1) {
    const key = String(index);
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (!descriptor) fail(`${name} must not contain empty entries`);
    if (!Object.hasOwn(descriptor, 'value')) fail(`${name} entries must be own data properties`);
    snapshot.push(descriptor.value);
  }
  return snapshot;
}

function requireInteger(value, name, minimum, maximum) {
  if (!Number.isInteger(value) || value < minimum || value > maximum) {
    fail(`${name} must be an integer between ${minimum} and ${maximum}`);
  }
  return value;
}

function normalizeSource(source) {
  requireRecord(source, 'source');
  rejectUnknownKeys(source, SOURCE_KEYS, 'source');

  const sha256 = requireOwnData(source, 'sha256', 'source');
  const bytesValue = requireOwnData(source, 'bytes', 'source');
  const format = requireOwnData(source, 'format', 'source');
  const widthValue = requireOwnData(source, 'width', 'source');
  const heightValue = requireOwnData(source, 'height', 'source');
  const hasAlpha = requireOwnData(source, 'hasAlpha', 'source');
  const isAnimated = requireOwnData(source, 'isAnimated', 'source');
  const orientationKnown = requireOwnData(source, 'orientationKnown', 'source');
  const orientationValue = requireOwnData(source, 'orientation', 'source');

  if (typeof sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(sha256)) {
    fail('source.sha256 must be a lowercase SHA-256 digest');
  }
  const bytes = requireInteger(bytesValue, 'source.bytes', 1, MAX_INPUT_BYTES);
  const width = requireInteger(widthValue, 'source.width', 1, MAX_DIMENSION);
  const height = requireInteger(heightValue, 'source.height', 1, MAX_DIMENSION);
  if (width * height > MAX_PIXELS) fail(`source dimensions must not exceed ${MAX_PIXELS} pixels`);
  if (!['jpeg', 'jpg', 'png', 'webp'].includes(format)) {
    fail('source.format must be jpeg, jpg, png, or webp');
  }
  if (typeof hasAlpha !== 'boolean') fail('source.hasAlpha must be a boolean');
  if (typeof isAnimated !== 'boolean') fail('source.isAnimated must be a boolean');
  if (isAnimated) fail('animated input is not supported by publicUpload');
  if (orientationKnown !== true) {
    fail('source.orientationKnown must be true before compiling an artifact plan');
  }

  const orientation = orientationValue === null
    ? null
    : requireInteger(orientationValue, 'source.orientation', 1, 8);
  const swapsDimensions = orientation != null && orientation >= 5;

  return {
    sha256,
    bytes,
    detectedFormat: format === 'jpg' ? 'jpeg' : format,
    encodedWidth: width,
    encodedHeight: height,
    displayWidth: swapsDimensions ? height : width,
    displayHeight: swapsDimensions ? width : height,
    hasAlpha,
    isAnimated: false,
    orientationKnown: true,
    orientation,
    orientationApplied: orientation != null && orientation !== 1,
  };
}

function normalizeWidths(rawWidths, sourceWidth) {
  if (!Array.isArray(rawWidths) || rawWidths.length === 0) fail('widths must be a non-empty array');
  if (rawWidths.length > MAX_RAW_WIDTHS) {
    fail(`widths must contain at most ${MAX_RAW_WIDTHS} raw entries`);
  }
  const widths = [...new Set(snapshotArray(rawWidths, 'widths')
    .map((width) => requireInteger(width, 'widths entry', 16, 8192)))]
    .sort((left, right) => left - right);
  if (widths.length > MAX_WIDTHS) fail(`widths must contain at most ${MAX_WIDTHS} entries`);
  const displayWidths = widths.filter((width) => width <= sourceWidth);
  if (displayWidths.length > 0) return displayWidths;
  if (sourceWidth < 16) fail('source display width must be at least 16 pixels');
  return [sourceWidth];
}

function normalizeFormats(rawFormats) {
  if (!Array.isArray(rawFormats) || rawFormats.length === 0) fail('formats must be a non-empty array');
  if (rawFormats.length > MAX_RAW_FORMATS) {
    fail(`formats must contain at most ${MAX_RAW_FORMATS} raw entries`);
  }
  const formats = snapshotArray(rawFormats, 'formats');
  for (const format of formats) {
    if (!FORMAT_ORDER.includes(format)) fail('formats entries must be jpeg, webp, or avif');
  }
  const requested = new Set(formats);
  if (requested.size > MAX_FORMATS) fail(`formats must contain at most ${MAX_FORMATS} entries`);
  return FORMAT_ORDER.filter((format) => requested.has(format));
}

function normalizeBudgets(rawBudgets, artifactIds) {
  if (!Array.isArray(rawBudgets)) fail('budgets must be an array');
  if (rawBudgets.length > MAX_ARTIFACTS) fail(`budgets must contain at most ${MAX_ARTIFACTS} entries`);

  const budgets = new Map();
  for (const rawBudget of snapshotArray(rawBudgets, 'budgets')) {
    requireRecord(rawBudget, 'budget');
    rejectUnknownKeys(rawBudget, BUDGET_KEYS, 'budget');
    const width = requireInteger(requireOwnData(rawBudget, 'width', 'budget'), 'budget.width', 16, 8192);
    const format = requireOwnData(rawBudget, 'format', 'budget');
    if (!FORMAT_ORDER.includes(format)) fail('budget.format must be jpeg, webp, or avif');
    const maxBytes = requireInteger(requireOwnData(rawBudget, 'maxBytes', 'budget'), 'budget.maxBytes', 1, MAX_BUDGET_BYTES);
    const id = `${width}/${format}`;
    if (!artifactIds.has(id)) fail(`budget target does not exist in the normalized artifact plan: ${id}`);
    if (budgets.has(id)) fail(`duplicate budget target: ${id}`);
    budgets.set(id, { width, format, maxBytes });
  }
  return [...artifactIds].flatMap((id) => budgets.has(id) ? [budgets.get(id)] : []);
}

function digest(value) {
  return createHash('sha256').update(JSON.stringify(value)).digest('hex');
}

function deepFreeze(value) {
  Object.freeze(value);
  for (const nested of Object.values(value)) {
    if (nested && typeof nested === 'object' && !Object.isFrozen(nested)) deepFreeze(nested);
  }
  return value;
}

function compileArtifactPlan(sourceInput, policyInput = {}, compilerFingerprint) {
  const source = normalizeSource(sourceInput);
  const policy = requireRecord(policyInput, 'policy');
  rejectUnknownKeys(policy, POLICY_KEYS, 'policy');
  const preset = optionalOwnData(policy, 'preset', 'policy');
  const widthsInput = optionalOwnData(policy, 'widths', 'policy');
  const formatsInput = optionalOwnData(policy, 'formats', 'policy');
  const budgetsInput = optionalOwnData(policy, 'budgets', 'policy');
  const placeholder = optionalOwnData(policy, 'placeholder', 'policy');
  if (preset !== undefined && preset !== 'publicUpload') fail('policy.preset must be publicUpload');
  if (placeholder !== undefined && typeof placeholder !== 'boolean') {
    fail('policy.placeholder must be a boolean');
  }
  if (typeof compilerFingerprint !== 'string' || !/^[a-f0-9]{64}$/.test(compilerFingerprint)) {
    fail('compilerFingerprint must be a lowercase SHA-256 digest');
  }

  const widths = normalizeWidths(widthsInput === undefined ? DEFAULT_WIDTHS : widthsInput, source.displayWidth);
  const formats = normalizeFormats(formatsInput === undefined ? ['webp'] : formatsInput);
  if (source.hasAlpha && formats.includes('jpeg')) fail('alpha input cannot produce jpeg artifacts');

  const artifactIds = new Set(formats.flatMap((format) => widths.map((width) => `${width}/${format}`)));
  if (artifactIds.size > MAX_ARTIFACTS) fail(`artifact plan must contain at most ${MAX_ARTIFACTS} entries`);
  const budgets = normalizeBudgets(budgetsInput === undefined ? [] : budgetsInput, artifactIds);
  const normalizedPolicy = {
    preset: 'publicUpload',
    widths,
    formats,
    budgets,
    placeholder: placeholder === undefined ? true : placeholder,
  };
  const policyFingerprint = digest(normalizedPolicy);
  const budgetsById = new Map(budgets.map((budget) => [`${budget.width}/${budget.format}`, budget.maxBytes]));
  const artifacts = formats.flatMap((format) => widths.map((width) => {
    const id = `${width}/${format}`;
    const targetBytes = budgetsById.get(id);
    return {
      id,
      path: `image-${width}.${format === 'jpeg' ? 'jpg' : format}`,
      format,
      width,
      quality: DEFAULT_QUALITY[format],
      ...(targetBytes == null ? {} : { targetBytes }),
    };
  }));
  const srcsets = formats.map((format) => ({
    format,
    value: artifacts
      .filter((artifact) => artifact.format === format)
      .map((artifact) => `${artifact.path} ${artifact.width}w`)
      .join(', '),
  }));

  return deepFreeze({
    schemaVersion: 1,
    compilerFingerprint,
    policyFingerprint,
    cacheKey: digest({ schemaVersion: 1, compilerFingerprint, sourceSha256: source.sha256, policyFingerprint }),
    source,
    policy: normalizedPolicy,
    artifacts,
    delivery: { srcsets },
    placeholder: normalizedPolicy.placeholder
      ? { id: 'placeholder', path: 'placeholder.webp', format: 'webp', width: 16, quality: 20 }
      : null,
  });
}

module.exports = { compileArtifactPlan };
