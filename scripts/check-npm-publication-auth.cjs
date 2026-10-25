// Read-only credential check. Never print registry response bodies, headers,
// token identifiers or the injected credential, including on request failure.
const fs = require('node:fs');
const pkg = require('../package.json');

async function check({ request = fetch, credential = process.env.NODE_AUTH_TOKEN } = {}) {
  const report = { checkedAt: new Date().toISOString(), sourceRevision: process.env.GITHUB_SHA,
    status: 'BLOCKED', identity: 'UNKNOWN', packagePublishAccess: [],
    directPublishScopeAnd2fa: 'UNKNOWN', credentialValueDisclosed: false,
    limitation: 'Read-only access checks do not establish granular direct-publish versus stage-only scope, expiry at publication, or 2FA bypass. Verify corresponding token settings without sharing its value.' };
  if (!credential) { report.reason = 'No injected credential'; return report; }
  async function get(endpoint) {
    try {
      const response = await request(`https://registry.npmjs.org/${endpoint}`, {
        method: 'GET', headers: { authorization: `Bearer ${credential}` },
        redirect: 'error', signal: AbortSignal.timeout(15000),
      });
      return { status: response.status, body: response.ok ? await response.json() : null };
    } catch { return { status: 'REQUEST_FAILED', body: null }; }
  }
  const identity = await get('-/whoami');
  report.identityHttpStatus = identity.status;
  if (identity.status !== 200 || typeof identity.body?.username !== 'string') {
    report.reason = 'Credential authentication was not established'; return report;
  }
  report.identity = 'VALID';
  // A published baseline is used because an absent candidate returns 404 even
  // with correct authorization. npm's status endpoint requires publish access.
  for (const name of [pkg.name, ...Object.keys(pkg.optionalDependencies), `${pkg.name}-wasm`]) {
    const result = await get(`-/package/${encodeURIComponent(name)}/version/1.5.0/status`);
    report.packagePublishAccess.push({ name, baselineVersion: '1.5.0', httpStatus: result.status,
      status: result.status === 200 ? 'CONFIRMED_BY_STATUS_ENDPOINT' : 'UNKNOWN' });
  }
  report.reason = 'Corresponding token direct-publish scope, expiry and noninteractive 2FA settings require metadata verification';
  return report;
}

if (require.main === module) check().then(report => {
  const file = process.argv[2];
  if (!file) throw new Error('Expected report output path');
  fs.writeFileSync(file, `${JSON.stringify(report, null, 2)}\n`);
  console.log(JSON.stringify(report));
  process.exitCode = report.status === 'PASS' ? 0 : 1;
}).catch(() => { console.error('Read-only npm authentication check failed; no credential details were recorded.'); process.exitCode = 1; });
module.exports = { check };
