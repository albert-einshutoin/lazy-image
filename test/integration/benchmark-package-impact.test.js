/**
 * Regression coverage for benchmark package/build-impact metadata.
 *
 * Backend bake-off artifacts should make it clear whether a benchmark harness
 * changed the production package contract or only used operator-provided tools.
 */

const assert = require('node:assert/strict');
const {
  buildPackageImpactMarkdown,
  collectBenchmarkPackageImpact,
} = require('../helpers/package-impact');

let passed = 0;
let failed = 0;

function report(name, error) {
  if (error) {
    console.log(`not ok - ${name}`);
    console.log(`   Error: ${error.message}`);
    failed += 1;
  } else {
    console.log(`ok - ${name}`);
    passed += 1;
  }
}

function test(name, fn) {
  try {
    fn();
    report(name);
  } catch (error) {
    report(name, error);
  }
}

console.log('=== Benchmark Package Impact Metadata Tests ===\n');

test('collects native package contract metadata for backend bake-offs', () => {
  const impact = collectBenchmarkPackageImpact({
    benchmarkTool: 'cjpegli',
    candidateBackend: 'jpegli',
  });

  assert.equal(impact.packageName, '@alberteinshutoin/lazy-image');
  assert.equal(impact.runtimePackageChanged, false);
  assert.equal(impact.productionBuildDependencyChanged, false);
  assert.equal(impact.publicApiChanged, false);
  assert.equal(impact.optionalDependencyCount, 6);
  assert.equal(impact.nativeTargetCount, 6);
  assert(impact.npmFiles.includes('index.js'));
  assert(impact.npmFiles.includes('README.md'));
  assert(impact.nativeTargets.includes('x86_64-pc-windows-msvc'));
  assert.match(impact.benchmarkToolingScope, /operator-provided benchmark CLI/);
});

test('renders cross-platform impact into Markdown artifacts', () => {
  const markdown = buildPackageImpactMarkdown(
    collectBenchmarkPackageImpact({
      benchmarkTool: 'avifenc',
      candidateBackend: 'libavif backend matrix',
    })
  ).join('\n');

  assert.match(markdown, /## Package \/ Build Impact/);
  assert.match(markdown, /Runtime package changed: no/);
  assert.match(markdown, /Production build dependency changed: no/);
  assert.match(markdown, /macOS impact: unchanged by this benchmark harness/);
  assert.match(markdown, /Linux impact: unchanged by this benchmark harness/);
  assert.match(markdown, /Windows impact: unchanged by this benchmark harness/);
  assert.match(markdown, /Future production switch evidence required:/);
});

console.log(`\nTests passed: ${passed}, failed: ${failed}`);
if (failed > 0) {
  process.exit(1);
}
