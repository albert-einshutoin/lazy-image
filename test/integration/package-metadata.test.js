const assert = require('assert');
const fs = require('node:fs');
const { resolveRoot } = require('../helpers/paths');

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`✅ ${name}`);
    passed += 1;
  } catch (err) {
    console.error(`❌ ${name}`);
    console.error(`   ${err.message}`);
    failed += 1;
  }
}

function countH2Headings(content) {
  return content.split('\n').filter((line) => line.startsWith('## ')).length;
}

function assertQuickStartWithinFirst80(content, label) {
  const lines = content.split('\n');
  const quickStartLine = lines.findIndex((line) => /^##\s+Quick Start/.test(line));
  assert(quickStartLine >= 0, `${label} should contain \"## Quick Start\" heading`);
  assert(
    quickStartLine < 80,
    `${label} Quick Start should be within first 80 lines, currently at ${quickStartLine + 1}`,
  );
}

function assertQuickStartOrdering(content, label) {
  const lines = content.split('\n');
  const quickStartLine = lines.findIndex((line) => /^##\s+Quick Start/.test(line));
  const chooserLine = lines.findIndex((line) => /^##\s+Choose lazy-image if/.test(line));
  const architectureLine = lines.findIndex((line) => /^##\s+Architecture Overview/.test(line));

  assert(quickStartLine >= 0, `${label} should contain \"## Quick Start\" heading`);
  assert(chooserLine >= 0, `${label} should contain \"## Choose lazy-image if / Choose sharp if\" heading`);
  assert(architectureLine >= 0, `${label} should contain \"## Architecture Overview\" heading`);
  assert(
    quickStartLine < chooserLine,
    `${label}: Quick Start should appear before Choose lazy-image guidance`,
  );
  assert(
    chooserLine < architectureLine,
    `${label}: Choose lazy-image guidance should appear before Architecture Overview`,
  );
}

function extractCargoKeywords(content) {
  const match = content.match(/^keywords\s*=\s*\[([^\]]*)\]/m);
  if (!match) {
    return [];
  }
  return match[1]
    .split(',')
    .map((item) => item.trim().replace(/^[\"']|[\"']$/g, ''))
    .filter(Boolean);
}

const packageJson = require(resolveRoot('package.json'));
const readmeMd = fs.readFileSync(resolveRoot('README.md'), 'utf8');
const readmeJaMd = fs.readFileSync(resolveRoot('README.ja.md'), 'utf8');
const cargoToml = fs.readFileSync(resolveRoot('Cargo.toml'), 'utf8');

test('package keywords include avif', () => {
  assert(
    Array.isArray(packageJson.keywords),
    'package.json should define an array of keywords',
  );
  assert(packageJson.keywords.includes('avif'), 'keywords should include \"avif\"');
  assert(
    packageJson.keywords.includes('image-optimization'),
    'keywords should include \"image-optimization\"',
  );
  assert(
    packageJson.keywords.includes('compression'),
    'keywords should include \"compression\"',
  );
});

test('package description is 80-250 chars', () => {
  assert(
    typeof packageJson.description === 'string',
    'package.json should define a string description',
  );
  assert(
    packageJson.description.length >= 80,
    'description should be at least 80 characters long',
  );
  assert(
    packageJson.description.length <= 250,
    'description should be at most 250 characters long',
  );
});

test('README and README.ja.md should keep Quick Start within first 80 lines', () => {
  assertQuickStartWithinFirst80(readmeMd, 'README.md');
  assertQuickStartWithinFirst80(readmeJaMd, 'README.ja.md');
});

test('README and README.ja.md should have matching ## heading count', () => {
  assert.strictEqual(
    countH2Headings(readmeMd),
    countH2Headings(readmeJaMd),
    'README.md and README.ja.md should have the same number of ## headings',
  );
});

test('README and README.ja.md should keep discovery flow ordering', () => {
  assertQuickStartOrdering(readmeMd, 'README.md');
  assertQuickStartOrdering(readmeJaMd, 'README.ja.md');
});

test('package description keeps Image Firewall positioning', () => {
  assert(
    /Image Firewall/.test(packageJson.description),
    'package description should keep Image Firewall positioning in discovery-facing copy',
  );
});

test('package keywords keep parity with cargo avif keyword', () => {
  const cargoKeywords = extractCargoKeywords(cargoToml);
  assert(
    cargoKeywords.includes('avif'),
    'Cargo.toml should keep `avif` keyword as core runtime story of truth',
  );
  assert(
    packageJson.keywords.includes('avif'),
    'package.json should keep `avif` keyword for package discoverability',
  );
});

if (failed > 0) {
  console.error(`\n❌ ${failed} metadata test(s) failed`);
  process.exit(1);
}

console.log(`\n✅ ${passed} package metadata integration tests passed`);
