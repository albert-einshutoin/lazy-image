/**
 * Public documentation contract checks for safety limits.
 *
 * These assertions intentionally read the Rust constants instead of copying
 * their values into JavaScript. Security documentation must fail when the
 * always-on decode limit and the published Image Firewall table drift apart.
 */

const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const ROOT = path.resolve(__dirname, '..', '..');
const engineSource = fs.readFileSync(path.join(ROOT, 'src', 'engine.rs'), 'utf8');
const architecture = fs.readFileSync(path.join(ROOT, 'docs', 'ARCHITECTURE.md'), 'utf8');

const maxPixelsMatch = engineSource.match(
  /pub const MAX_PIXELS:\s*u64\s*=\s*([\d_]+);/
);
assert(maxPixelsMatch, 'src/engine.rs must declare MAX_PIXELS');

const maxPixels = Number(maxPixelsMatch[1].replaceAll('_', ''));
const maxPixelsInMegapixels = maxPixels / 1_000_000;

assert.equal(maxPixels, 100_000_000, 'global decode limit changed; review the public contract');
assert.match(
  architecture,
  new RegExp(`\\| Max pixels \\| 40 MP\\s+\\| 75 MP\\s+\\| ${maxPixelsInMegapixels} MP`),
  'No firewall column must match the always-on MAX_PIXELS value'
);
assert.doesNotMatch(
  architecture,
  /\b1 GP\b/,
  'architecture must not claim a 1 GP limit when decode rejects above 100 MP'
);
assert.match(
  architecture,
  /always-on.*100 MP.*32,768/si,
  'architecture must explain that global pixel and dimension limits stay enabled'
);

console.log('docs safety-limit contract passed');
