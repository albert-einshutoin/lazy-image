import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import assert from 'node:assert/strict';
export const here = path.dirname(fileURLToPath(import.meta.url));
export const sha = file => createHash('sha256').update(fs.readFileSync(file)).digest('hex');
export function loadPublished() {
  for (const key of ['NAPI_RS_NATIVE_LIBRARY_PATH', 'NODE_PATH', 'NODE_OPTIONS']) assert.ok(!process.env[key], key);
  const require = createRequire(import.meta.url);
  const spec = JSON.parse(fs.readFileSync(path.join(here, 'identity.json')));
  assert.equal(sha(path.join(here, 'package-lock.json')), spec.lockSha256);
  const root = fs.realpathSync(path.join(here, 'node_modules/@alberteinshutoin/lazy-image'));
  const entry = fs.realpathSync(require.resolve('@alberteinshutoin/lazy-image'));
  assert.ok(entry.startsWith(root + path.sep), 'must load this consumer installation');
  assert.equal(JSON.parse(fs.readFileSync(path.join(root, 'package.json'))).version, '1.5.0');
  const api = require('@alberteinshutoin/lazy-image');
  assert.equal(api.version(), '1.5.0');
  const bindings = Object.keys(require.cache).filter(file => file.endsWith('.node'));
  assert.equal(bindings.length, 1);
  assert.equal(sha(bindings[0]), spec.nativeSha256);
  for (const js of spec.loadedJs) assert.equal(sha(path.join(root, js.file)), js.sha256);
  return { api, identity: { version: api.version(), sourceSha: spec.sourceSha,
    nativeSha256: sha(bindings[0]), lockSha256: spec.lockSha256, loadedJs: spec.loadedJs,
    platform: process.platform, arch: process.arch, node: process.version, overrides: [] } };
}
