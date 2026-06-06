const crypto = require('node:crypto');
const fs = require('node:fs');
const path = require('node:path');
const sharp = require('sharp');
const { resolveRoot } = require('./paths');

function sha256(buffer) {
  return crypto.createHash('sha256').update(buffer).digest('hex');
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
};
