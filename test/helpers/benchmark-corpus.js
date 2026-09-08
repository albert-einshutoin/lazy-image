const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const { resolveRoot } = require('./paths');

const CORPUS_MANIFEST_SCHEMA_VERSION = 1;
const MANIFEST_KEYS = new Set(['schemaVersion', 'name', 'license', 'entries']);
const LICENSE_KEYS = new Set(['spdx', 'path', 'sha256']);
const ENTRY_KEYS = new Set([
  'id', 'path', 'bytes', 'sha256', 'license', 'category', 'source', 'expectations', 'policy',
]);

function sha256(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
}

function requireRecord(value, name) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)
    || (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null)) {
    throw new Error(`${name} must be an object`);
  }
  return value;
}

function rejectUnknownKeys(value, allowed, name) {
  for (const key of Reflect.ownKeys(value)) {
    if (typeof key !== 'string' || !allowed.has(key)) throw new Error(`unknown ${name} key: ${String(key)}`);
  }
}

function resolveManifestPath(root, relativePath, name) {
  if (typeof relativePath !== 'string' || relativePath.length === 0
    || path.posix.isAbsolute(relativePath)
    || relativePath.includes('\\')
    || path.posix.normalize(relativePath) !== relativePath) {
    throw new Error(`${name} must be a normalized relative POSIX path`);
  }
  const rootPath = path.resolve(root);
  const resolved = path.resolve(rootPath, ...relativePath.split('/'));
  const relative = path.relative(rootPath, resolved);
  if (!relative || relative.startsWith('..') || path.isAbsolute(relative)) {
    throw new Error(`${name} must stay within the corpus root`);
  }
  return resolved;
}

function readRegularManifestFile(root, relativePath, name) {
  const filePath = resolveManifestPath(root, relativePath, name);
  let stat;
  try {
    stat = fs.lstatSync(filePath);
  } catch (error) {
    throw new Error(`${name} does not exist: ${error.message}`);
  }
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error(`${name} must be a regular file`);
  return { path: filePath, data: fs.readFileSync(filePath) };
}

function readCorpusManifest(manifestPath = resolveRoot('test/benchmarks/corpus/manifest.json')) {
  try {
    return JSON.parse(fs.readFileSync(manifestPath, 'utf8'));
  } catch (error) {
    throw new Error(`benchmark corpus manifest could not be read: ${error.message}`);
  }
}

function verifyCorpusManifest(manifest, root = resolveRoot()) {
  requireRecord(manifest, 'benchmark corpus manifest');
  rejectUnknownKeys(manifest, MANIFEST_KEYS, 'manifest');
  if (manifest.schemaVersion !== CORPUS_MANIFEST_SCHEMA_VERSION) {
    throw new Error(`unsupported benchmark corpus manifest schema: ${manifest.schemaVersion}`);
  }
  if (typeof manifest.name !== 'string' || manifest.name.length === 0) {
    throw new Error('benchmark corpus manifest.name is required');
  }

  const license = requireRecord(manifest.license, 'manifest.license');
  rejectUnknownKeys(license, LICENSE_KEYS, 'license');
  if (typeof license.spdx !== 'string' || !/^[A-Za-z0-9][A-Za-z0-9.+-]*$/.test(license.spdx)) {
    throw new Error('manifest.license.spdx must be an SPDX identifier');
  }
  if (typeof license.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(license.sha256)) {
    throw new Error('manifest.license.sha256 must be a lowercase SHA-256 digest');
  }
  const licenseFile = readRegularManifestFile(root, license.path, 'manifest.license.path');
  const licenseDigest = sha256(licenseFile.data);
  if (licenseDigest !== license.sha256) {
    throw new Error('manifest license checksum does not match the referenced license file');
  }

  if (!Array.isArray(manifest.entries) || manifest.entries.length === 0) {
    throw new Error('benchmark corpus manifest.entries must not be empty');
  }
  const ids = new Set();
  const paths = new Set();
  const entries = manifest.entries.map((entry, index) => {
    requireRecord(entry, `manifest.entries[${index}]`);
    rejectUnknownKeys(entry, ENTRY_KEYS, `entry[${index}]`);
    if (typeof entry.id !== 'string' || !/^[a-z0-9][a-z0-9-]*$/.test(entry.id) || ids.has(entry.id)) {
      throw new Error(`manifest.entries[${index}].id must be unique and slug-like`);
    }
    ids.add(entry.id);
    if (entry.license !== license.spdx) throw new Error(`manifest.entries[${index}] license does not match manifest license`);
    if (typeof entry.category !== 'string' || entry.category.length === 0) {
      throw new Error(`manifest.entries[${index}].category is required`);
    }
    if (!Number.isSafeInteger(entry.bytes) || entry.bytes < 1) {
      throw new Error(`manifest.entries[${index}].bytes must be a positive integer`);
    }
    if (typeof entry.sha256 !== 'string' || !/^[a-f0-9]{64}$/.test(entry.sha256)) {
      throw new Error(`manifest.entries[${index}].sha256 must be a lowercase SHA-256 digest`);
    }
    if (paths.has(entry.path)) throw new Error(`manifest.entries[${index}].path must be unique`);
    paths.add(entry.path);
    if (!requireRecord(entry.source, `manifest.entries[${index}].source`).kind
      || typeof entry.source.kind !== 'string') {
      throw new Error(`manifest.entries[${index}].source.kind is required`);
    }
    requireRecord(entry.expectations, `manifest.entries[${index}].expectations`);
    requireRecord(entry.policy, `manifest.entries[${index}].policy`);

    const file = readRegularManifestFile(root, entry.path, `manifest.entries[${index}].path`);
    if (file.data.length !== entry.bytes) throw new Error(`checksum byte count mismatch for ${entry.id}`);
    const digest = sha256(file.data);
    if (digest !== entry.sha256) throw new Error(`checksum mismatch for ${entry.id}`);
    return { ...entry, absolutePath: file.path };
  });

  return {
    schemaVersion: manifest.schemaVersion,
    name: manifest.name,
    license: { ...license, bytes: licenseFile.data.length },
    entries,
  };
}

function relativeRoot(filePath) {
  return path.relative(resolveRoot(), filePath).replace(/\\/g, '/');
}

function describeSource({ sourcePath, sourceKind, generatedBy }) {
  if (!sourcePath) {
    return {
      kind: sourceKind || 'generated',
      generator: generatedBy || sourceKind || 'unknown',
      path: null,
      bytes: null,
      sha256: null,
    };
  }

  const data = fs.readFileSync(sourcePath);
  return {
    kind: sourceKind || 'fixture',
    path: relativeRoot(sourcePath),
    bytes: data.length,
    sha256: sha256(data),
  };
}

async function describeReferenceCorpusEntry({
  label,
  sourcePath = null,
  sourceKind = 'fixture',
  generatedBy = null,
  referencePng,
  referencePngPath,
  settings = {},
  expectations = {},
}) {
  const metadata = await sharp(referencePng).metadata();

  return {
    label,
    source: describeSource({ sourcePath, sourceKind, generatedBy }),
    referencePng: {
      path: relativeRoot(referencePngPath),
      bytes: referencePng.length,
      sha256: sha256(referencePng),
      width: metadata.width ?? null,
      height: metadata.height ?? null,
      channels: metadata.channels ?? null,
      hasAlpha: Boolean(metadata.hasAlpha),
      iccBytes: metadata.icc ? metadata.icc.length : 0,
    },
    settings,
    expectations,
  };
}

function formatKeyValues(value) {
  const entries = Object.entries(value || {});
  if (entries.length === 0) return 'n/a';
  return entries.map(([key, item]) => `${key}=${item}`).join(', ');
}

function markdownCell(value) {
  return String(value ?? 'n/a').replace(/\|/g, '\\|').replace(/\s+/g, ' ').trim();
}

function shortSha(value) {
  return value ? value.slice(0, 12) : 'n/a';
}

function sourceLabel(source) {
  if (source.path) return source.path;
  return source.generator || source.kind || 'generated';
}

function buildCorpusManifestMarkdown(corpus) {
  const lines = [
    '## Corpus Manifest',
    '',
    '| Case | Source | Source bytes | Source SHA-256 | Reference PNG | Reference size | Reference SHA-256 | Settings | Expectations |',
    '|---|---|---:|---|---|---|---|---|---|',
  ];

  for (const entry of corpus) {
    lines.push(
      [
        entry.label,
        sourceLabel(entry.source),
        entry.source.bytes ?? 'n/a',
        shortSha(entry.source.sha256),
        entry.referencePng.path,
        `${entry.referencePng.width}x${entry.referencePng.height}`,
        shortSha(entry.referencePng.sha256),
        formatKeyValues(entry.settings),
        formatKeyValues(entry.expectations),
      ]
        .map(markdownCell)
        .join(' | ')
        .replace(/^/, '| ')
        .replace(/$/, ' |')
    );
  }

  return lines;
}

module.exports = {
  buildCorpusManifestMarkdown,
  describeReferenceCorpusEntry,
  readCorpusManifest,
  sha256,
  verifyCorpusManifest,
};
