'use strict';

const assert = require('node:assert/strict');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { loadConfig } = require('../../ci/lib/config');
const { countMatchingRustTests } = require('../../ci/lib/test-targets');

const root = path.join(__dirname, '../..');
const config = loadConfig(root);
const result = spawnSync('cargo', ['test', '--no-default-features', '--', '--list'], {
  cwd: root,
  encoding: 'utf8',
  maxBuffer: 16 * 1024 * 1024,
});
assert.equal(result.status, 0, result.stderr);

for (const module of config.modules) {
  for (const target of module.unitTests || []) {
    if (!target.startsWith('rust::')) continue;
    const filter = target.slice(6);
    assert.ok(countMatchingRustTests(filter, result.stdout) > 0, `${filter} must select Rust tests`);
  }
}

console.log('selective Rust target tests passed');
