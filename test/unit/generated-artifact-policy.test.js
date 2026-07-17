'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const root = path.resolve(__dirname, '../..')
const packageJson = require(path.join(root, 'package.json'))
const packageLock = require(path.join(root, 'package-lock.json'))
const workflow = fs
  .readFileSync(path.join(root, '.github/workflows/CI.yml'), 'utf8')
  .replace(/\r\n/g, '\n')

assert.equal(
  packageJson.devDependencies['@napi-rs/cli'],
  '3.7.2',
  'the generator must be pinned exactly in package.json',
)
assert.equal(
  packageLock.packages[''].devDependencies['@napi-rs/cli'],
  '3.7.2',
  'the root lockfile contract must preserve the exact generator version',
)
assert.equal(
  packageJson.scripts['check:generated-artifacts'],
  'git diff --exit-code -- index.js index.d.ts',
)
assert.match(workflow, /node-version: 20/)
assert.match(
  workflow,
  /Verify committed NAPI artifacts are reproducible[\s\S]*npm run check:generated-artifacts/,
)
assert.match(
  workflow,
  /Verify package after canonical generation[\s\S]*npm pack --dry-run/,
)

console.log('generated artifact policy test passed')
