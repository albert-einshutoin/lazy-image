const assert = require('assert');
const { spawnSync } = require('child_process');
const path = require('path');

// Minimal test to ensure cow-debug logging can be enabled via env + feature flag build.
// This test runs a tiny script that triggers a copy (resize) and checks stderr contains the marker.

const root = path.resolve(__dirname, '..', '..');
const script = `
  const { ImageEngine } = require('../index');
  const fs = require('fs');
  const img = fs.readFileSync(require('path').join(__dirname, '../fixtures/test_input.jpg'));
  const engine = ImageEngine.from(img).resize(10, 10, 'fill');
  (async () => { await engine.toBuffer('jpeg', 80); })();
`;

const result = spawnSync('node', ['-e', script], {
  cwd: root,
  env: {
    ...process.env,
    LAZY_IMAGE_DEBUG_COW: '1',
  },
  encoding: 'utf8',
});

// If feature cow-debug is not compiled, tracing output won't appear; we still require process exit 0.
assert.strictEqual(result.status, 0, `process failed: ${result.stderr}`);

// When cow-debug feature is enabled, tracing::debug! should emit the marker substring.
// We only check presence conditionally to keep test passing for default builds without that feature.
if (process.env.CI_COW_DEBUG === '1') {
  assert(
    /lazy_image::cow/.test(result.stderr),
    `expected tracing debug output in stderr, got: ${result.stderr}`
  );
}

console.log('cow-debug logging smoke test passed');
