'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const {
  buildAuditCommands,
  loadAuditPolicy,
  validateAuditPolicy,
} = require('../../scripts/cargo-audit-policy')

const repositoryRoot = path.join(__dirname, '../..')

function test(name, fn) {
  try {
    fn()
    console.log(`ok - ${name}`)
  } catch (error) {
    console.error(`not ok - ${name}`)
    console.error(error)
    process.exitCode = 1
  }
}

test('audits root and fuzz lockfiles with lockfile-scoped exceptions', () => {
  const policy = loadAuditPolicy(repositoryRoot)
  const commands = buildAuditCommands(policy, new Date('2026-07-19T00:00:00Z'))

  assert.equal(commands.length, 2)
  assert.deepEqual(
    commands.map(({ args }) => args.slice(0, 4)),
    [
      ['audit', '--deny', 'unmaintained', '--deny'],
      ['audit', '--file', 'fuzz/Cargo.lock', '--deny'],
    ],
  )

  assert.ok(commands.every(({ args }) => args.includes('unsound')))
  assert.deepEqual(
    commands[0].args.filter((argument) => argument.startsWith('RUSTSEC-')),
    ['RUSTSEC-2024-0436'],
  )
  assert.deepEqual(
    commands[1].args.filter((argument) => argument.startsWith('RUSTSEC-')),
    ['RUSTSEC-2024-0436'],
  )
})

test('rejects expired and malformed audit exceptions', () => {
  const validException = {
    advisory: 'RUSTSEC-2024-0436',
    package: 'paste',
    reason: 'Blocked by the production AVIF dependency path.',
    impact: 'Compile-time proc macro only.',
    remediation: 'Upgrade the AVIF dependency path.',
    owner: 'lazy-image maintainers',
    upstream: 'https://github.com/xiph/rav1e/issues/3418',
    reviewBy: '2026-10-31',
    lockfiles: ['Cargo.lock'],
  }

  assert.throws(
    () =>
      validateAuditPolicy(
        { exceptions: [{ ...validException, reviewBy: '2026-07-18' }] },
        new Date('2026-07-19T00:00:00Z'),
      ),
    /expired/,
  )
  assert.throws(
    () =>
      validateAuditPolicy(
        { exceptions: [{ ...validException, upstream: 'not-a-url' }] },
        new Date('2026-07-19T00:00:00Z'),
      ),
    /upstream/,
  )
  assert.throws(
    () =>
      validateAuditPolicy(
        { exceptions: [{ ...validException, reviewBy: '2026-13-40' }] },
        new Date('2026-07-19T00:00:00Z'),
      ),
    /reviewBy/,
  )
})

test('security workflow delegates Cargo auditing to the tested policy runner', () => {
  const workflow = fs.readFileSync(
    path.join(repositoryRoot, '.github/workflows/security.yml'),
    'utf8',
  )

  assert.match(workflow, /node scripts\/cargo-audit-policy\.js/)
  assert.doesNotMatch(workflow, /--ignore RUSTSEC-/)
})
