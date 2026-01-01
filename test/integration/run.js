const { spawnSync } = require('child_process');
const path = require('path');
const fs = require('fs');
const { ROOT_DIR, TEST_DIR } = require('../helpers/paths');

// Automatically find all .test.js files in the integration directory
const INTEGRATION_DIR = path.join(TEST_DIR, 'integration');
const SPEC_FILES = fs.readdirSync(INTEGRATION_DIR)
  .filter(file => file.endsWith('.test.js'))
  .map(file => path.join('test/integration', file));

let failures = 0;

console.log(`Found ${SPEC_FILES.length} test files`);

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

console.log('\n✅ All JS integration tests completed');
