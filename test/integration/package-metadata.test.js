/**
 * Package metadata contract checks for npm discoverability and onboarding clarity.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..', '..');
const packageJson = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));
const cargoToml = fs.readFileSync(path.join(ROOT, 'Cargo.toml'), 'utf8');
const readmeMd = fs.readFileSync(path.join(ROOT, 'README.md'), 'utf8');
const readmeJa = fs.readFileSync(path.join(ROOT, 'README.ja.md'), 'utf8');

function readmeHeadlineCount(content) {
  return (content.match(/^##\s+.+$/gm) || []).length;
}

function extractTomlArray(content, key) {
  const match = content.match(new RegExp(`^${key}\\s*=\\s*\\[([^\\]]*)\\]`, 'm'));
  if (!match) return [];
  return (match[1] || '')
    .split(',')
    .map((item) => item.trim().replace(/^['\"]|['\"]$/g, ''))
    .filter(Boolean);
}

function extractTomlString(content, key) {
  const match = content.match(new RegExp(`^${key}\\s*=\\s*['\"]([^'\"]+)['\"]`, 'm'));
  return match ? match[1] : '';
}

let passed = 0;
let failed = 0;

function test(name, fn) {
  try {
    fn();
    console.log(`✅ ${name}`);
    passed += 1;
  } catch (error) {
    console.log(`❌ ${name}`);
    console.error(`   ${error.message}`);
    failed += 1;
  }
}

(async () => {
  test('package keywords include discoverability terms', () => {
    const keywords = packageJson.keywords || [];
    assert(keywords.includes('avif'), 'package.json keywords must include avif');
    assert(keywords.includes('image-optimization'), 'package.json keywords must include image-optimization');
    assert(keywords.includes('compress'), 'package.json keywords must include compress');
    assert(keywords.includes('compression'), 'package.json keywords must include compression');
    assert(keywords.includes('image-processing'), 'package.json keywords must include image-processing');
    assert(keywords.includes('thumbnail'), 'package.json keywords must include thumbnail');
    assert(keywords.includes('exif'), 'package.json keywords must include exif');
  });

  test('package description length stays within policy-friendly bounds', () => {
    assert(
      typeof packageJson.description === 'string',
      'package.json description must be a string',
    );
    assert(
      packageJson.description.length >= 80,
      'package.json description should be at least 80 characters',
    );
    assert(
      packageJson.description.length <= 250,
      'package.json description should be at most 250 characters',
    );
  });

  test('README.md has Quick Start section in first 80 lines', () => {
    const first80 = readmeMd.split('\n').slice(0, 80);
    const hasQuickStart = first80.some((line) => line.includes('## Quick Start'));
    assert(hasQuickStart, 'README.md should include ## Quick Start in above-the-fold area');
  });

  test('README heading structure is synchronized with README.ja.md', () => {
    const readmeHeadings = readmeHeadlineCount(readmeMd);
    const readmeJaHeadings = readmeHeadlineCount(readmeJa);
    assert(
      readmeHeadings === readmeJaHeadings,
      `README.md has ${readmeHeadings} ## headings, README.ja.md has ${readmeJaHeadings}`,
    );
  });

  test('Cargo package metadata includes security/positioning and avif', () => {
    const cargoKeywords = extractTomlArray(cargoToml, 'keywords');
    const cargoDescription = extractTomlString(cargoToml, 'description');

    assert(cargoKeywords.length <= 5, 'Cargo.toml keywords must be 5 or fewer entries');
    assert(cargoKeywords.includes('avif'), 'Cargo.toml keywords must include avif');
    assert(
      cargoKeywords.includes('image-optimization'),
      'Cargo.toml keywords should include image-optimization',
    );
    assert(
      /secure|security/i.test(cargoDescription),
      'Cargo.toml description should mention security posture',
    );
    assert(
      /mozjpeg/.test(cargoDescription),
      'Cargo.toml description should mention mozjpeg position as a positioning anchor',
    );
  });

  console.log(`\n✅ package metadata checks completed: ${passed} passed, ${failed} failed`);
  if (failed > 0) {
    process.exit(1);
  }
})();
