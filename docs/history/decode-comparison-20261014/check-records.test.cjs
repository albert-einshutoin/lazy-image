const fs = require('node:fs'), path = require('node:path'), os = require('node:os');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const { spawnSync } = require('node:child_process');
const repository = path.resolve(__dirname, '../../..');
const temp = fs.mkdtempSync(path.join(os.tmpdir(), 'decode-record-check-'));
try {
  const copy = path.join(temp, 'history'); fs.cpSync(__dirname, copy, { recursive: true });
  const run = () => spawnSync(process.execPath, [path.join(__dirname, 'check-records.cjs'), repository, copy], { encoding: 'utf8' });
  const good = run(); assert.equal(good.status, 0, good.stderr);
  const cases = [
    ['wrong-selected-image', 'evidence/browser/baseline-single-webp-1.json', d => { d.qa.images[0].currentSrc = 'http://127.0.0.1:8877/reference/coffee-64.png'; }],
    ['error-hidden-as-success', 'evidence/browser/diagnostic-control-invalid-1.json', d => { d.qa.images[2].status = 'success'; }],
    ['body-hash-mismatch', 'evidence/browser/diagnostic-sheet-1.json', d => { d.qa.probes[0].sha256 = '0'.repeat(64); }],
    ['event-missing', 'evidence/browser/diagnostic-sheet-1.json', d => { d.qa.events.pop(); }],
    ['wrong-source-revision', 'evidence/diagnostic-binding.json', d => { d.evaluationCommit = '0'.repeat(40); }],
  ];
  for (const [name, relative, mutate] of cases) {
    const file = path.join(copy, relative), original = fs.readFileSync(file); const data = JSON.parse(original); mutate(data);
    const ledgerFile = path.join(copy, 'LOG_REDACTIONS.json'), ledgerBytes = fs.readFileSync(ledgerFile), ledger = JSON.parse(ledgerBytes);
    const changed = Buffer.from(JSON.stringify(data)); fs.writeFileSync(file, changed);
    // Reach semantic checks after accepting the altered disposable copy's byte hash.
    const binding = ledger.files.find(f => f.file === relative);
    binding.publicBytes = changed.length; binding.publicSha256 = crypto.createHash('sha256').update(changed).digest('hex');
    fs.writeFileSync(ledgerFile, JSON.stringify(ledger));
    assert.equal(run().status, 1, name); fs.writeFileSync(file, original); fs.writeFileSync(ledgerFile, ledgerBytes);
  }
  console.log(JSON.stringify({ validExit: 0, rejectedTampering: cases.map(c => c[0]), passed: 6 }));
} finally { fs.rmSync(temp, { recursive: true, force: true }); }
