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

function headingIndex(markdown, heading) {
  return markdown.split('\n').findIndex((line) => line.trim() === heading);
}

function parseCargoKeywords(manifest) {
  const match = manifest.match(/^keywords\s*=\s*\[([^\]]+)\]/m);
  assert(match, 'Cargo.toml should define package keywords');
  return match[1]
    .split(',')
    .map((value) => value.trim().replace(/^"|"$/g, ''))
    .filter(Boolean);
}

const packageJson = require(path.join(ROOT_DIR, 'package.json'));

test('package metadata and README structure', () => {
  assert(Array.isArray(packageJson.keywords), 'package.json.keywords should be an array');
  for (const keyword of ['avif', 'image-optimization', 'image-processing', 'compress', 'thumbnail', 'exif']) {
    assert(
      packageJson.keywords.includes(keyword),
      `package.json keywords should include ${keyword}`,
    );
  }
  assert(
    packageJson.description.length >= 80 && packageJson.description.length <= 250,
    `package.json description length must be 80-250 chars (actual: ${packageJson.description.length})`,
  );
  assert(
    packageJson.description.includes('Image Firewall'),
    'package.json description should mention Image Firewall',
  );

  const cargoToml = readText('Cargo.toml');
  const cargoKeywords = parseCargoKeywords(cargoToml);
  assert(cargoKeywords.includes('avif'), 'Cargo.toml keywords should include avif');

  const readmeEn = readText('README.md');
  const readmeJa = readText('README.ja.md');

  assert(containsQuickStart(readmeEn), 'README.md should include "Quick Start" within first 80 lines');
  assert(
    headingIndex(readmeEn, '## Quick Start (5 lines)') < headingIndex(readmeEn, '## Choose lazy-image if / Choose sharp if'),
    'README.md should place the chooser after Quick Start',
  );
  assert(
    headingIndex(readmeEn, '## Choose lazy-image if / Choose sharp if') < headingIndex(readmeEn, '## Architecture Overview'),
    'README.md should place adoption guidance before architecture details',
  );
  assert(
    headingIndex(readmeJa, '## Quick Start (5 lines)') < headingIndex(readmeJa, '## Choose lazy-image if / Choose sharp if'),
    'README.ja.md should place the chooser after Quick Start',
  );
  assert(
    headingIndex(readmeJa, '## Choose lazy-image if / Choose sharp if') < headingIndex(readmeJa, '## Architecture Overview'),
    'README.ja.md should place adoption guidance before architecture details',
  );
  assert(
    countHeadings(readmeEn) === countHeadings(readmeJa),
    `README heading count mismatch: README.md=${countHeadings(readmeEn)} README.ja.md=${countHeadings(readmeJa)}`,
  );
});
