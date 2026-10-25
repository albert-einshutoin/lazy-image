const assert = require('node:assert/strict');
const { check } = require('../../scripts/check-npm-publication-auth.cjs');
(async () => {
  const secret = 'test-secret-never-disclose';
  const calls = [];
  const request = async (url, options) => {
    calls.push(url);
    assert.equal(options.method, 'GET');
    assert.equal(options.redirect, 'error');
    assert.equal(options.headers.authorization, `Bearer ${secret}`);
    return { ok: true, status: 200, json: async () => url.endsWith('whoami') ? { username: 'owner', token: secret } : { status: 'published', token: secret } };
  };
  const report = await check({ request, credential: secret });
  assert.equal(calls.length, 9);
  assert.equal(report.identity, 'VALID');
  assert.equal(report.packagePublishAccess.length, 8);
  assert.equal(report.status, 'BLOCKED', 'identity and access must not be promoted to direct-publish authorization');
  assert.equal(JSON.stringify(report).includes(secret), false);
  const failure = await check({ request: async () => { throw new Error(secret); }, credential: secret });
  assert.equal(JSON.stringify(failure).includes(secret), false);
  assert.equal(failure.identity, 'UNKNOWN');
  console.log('read-only npm auth check preserves unknown scope and excludes credentials on success/failure');
})().catch(error => { console.error(error); process.exitCode = 1; });
