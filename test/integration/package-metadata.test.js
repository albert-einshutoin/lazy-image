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

const packageJson = require(resolveRoot('package.json'));
const readmeMd = fs.readFileSync(resolveRoot('README.md'), 'utf8');
const readmeJaMd = fs.readFileSync(resolveRoot('README.ja.md'), 'utf8');
const readmeMdLines = readmeMd.split('\n');

test('package keywords include avif', () => {
  assert(
    Array.isArray(packageJson.keywords),
    'package.json should define an array of keywords',
  );
  assert(packageJson.keywords.includes('avif'), 'keywords should include "avif"');
  assert(
    packageJson.keywords.includes('image-optimization'),
    'keywords should include "image-optimization"',
  );
  assert(
    packageJson.keywords.includes('compression'),
    'keywords should include "compression"',
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

test('README has Quick Start in first 80 lines', () => {
  const quickStartLine = readmeMdLines.findIndex((line) => /^##\s+Quick Start/.test(line));
  assert(quickStartLine >= 0, 'README.md should contain "## Quick Start" heading');
  assert(
    quickStartLine < 80,
    `Quick Start should be within first 80 lines, currently at ${quickStartLine + 1}`,
  );
});

test('README and README.ja.md should have matching ## heading count', () => {
  assert.strictEqual(
    countH2Headings(readmeMd),
    countH2Headings(readmeJaMd),
    'README.md and README.ja.md should have the same number of ## headings',
  );
});

if (failed > 0) {
  console.error(`\n❌ ${failed} metadata test(s) failed`);
  process.exit(1);
}

console.log(`\n✅ ${passed} package metadata integration tests passed`);
