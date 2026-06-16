/**
 * Run every example in fixture-backed self-test mode.
 */

import { spawnSync } from 'node:child_process';
import path from 'node:path';
import { EXAMPLES_DIR, ROOT_DIR } from './_load.mjs';

const EXAMPLES = [
  'basic.mjs',
  'batch-processing.mjs',
  'error-handling.mjs',
  'upload-sanitize-server.mjs',
  'build-time-optimize.mjs',
  'metrics-observability.mjs',
];

let failures = 0;

for (const example of EXAMPLES) {
  const file = path.join(EXAMPLES_DIR, example);
  console.log(`\nRunning ${example} --self-test`);

  const result = spawnSync(process.execPath, [file, '--self-test'], {
    cwd: ROOT_DIR,
    stdio: 'inherit',
    env: process.env,
  });

  if (result.status !== 0) {
    failures += 1;
  }
}

if (failures > 0) {
  console.error(`\n${failures} example self-test(s) failed`);
  process.exit(1);
}

console.log('\nAll example self-tests passed');
