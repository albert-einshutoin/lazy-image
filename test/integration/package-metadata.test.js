const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { ROOT_DIR } = require('../helpers/paths');

function test(name, fn) {
  try {
    fn();
    console.log(`✅ ${name}`);
  } catch (error) {
    console.log(`❌ ${name}`);
    throw error;
  }
}

function readText(fileName) {
  return fs.readFileSync(path.join(ROOT_DIR, fileName), 'utf8');
}

function countHeadings(markdown) {
  return markdown.split('\n').filter((line) => line.startsWith('## ')).length;
}

function containsQuickStart(markdown) {
  return markdown.split('\n').slice(0, 80).some((line) => line.includes('Quick Start'));
}

const packageJson = require(path.join(ROOT_DIR, 'package.json'));

test('package metadata and README structure', () => {
  assert(Array.isArray(packageJson.keywords), 'package.json.keywords should be an array');
  assert(
    packageJson.keywords.includes('avif'),
    'package.json keywords should include avif',
  );
  assert(
    packageJson.keywords.includes('image-optimization'),
    'package.json keywords should include image-optimization',
  );
  assert(
    packageJson.description.length >= 80 && packageJson.description.length <= 250,
    `package.json description length must be 80-250 chars (actual: ${packageJson.description.length})`,
  );

  const readmeEn = readText('README.md');
  const readmeJa = readText('README.ja.md');

  assert(containsQuickStart(readmeEn), 'README.md should include "Quick Start" within first 80 lines');
  assert(
    countHeadings(readmeEn) === countHeadings(readmeJa),
    `README heading count mismatch: README.md=${countHeadings(readmeEn)} README.ja.md=${countHeadings(readmeJa)}`,
  );
});

