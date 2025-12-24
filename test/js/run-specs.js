const { spawnSync } = require('child_process');
const path = require('path');
const { ROOT_DIR } = require('./helpers/paths');

const SPEC_FILES = [
  'test/js/specs/basic.test.js',
  'test/js/specs/edge-cases.test.js',
  'test/js/specs/concurrency-validation.test.js',
  'test/js/specs/deprecation-warning.test.js',
  'test/js/specs/thread-pool-env.test.js',
  'test/js/specs/type-improvements.test.js',
  'test/js/specs/zero-copy-safety.test.js',
];

let failures = 0;

for (const file of SPEC_FILES) {
  const fullPath = path.join(ROOT_DIR, file);
  console.log(`\n▶️  Running ${file}`);
  const result = spawnSync('node', [fullPath], { stdio: 'inherit' });
  if (result.status !== 0) {
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`\n❌ ${failures} test file(s) failed`);
  process.exit(1);
}

console.log('\n✅ All JS spec files completed');
