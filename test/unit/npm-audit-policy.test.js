'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const { runNpmAudit } = require('../../scripts/npm-audit-policy')

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

const serviceUnavailable = {
  status: 1,
  stdout: '',
  stderr: [
    'npm warn audit 503 Service Unavailable',
    'npm error audit endpoint returned an error',
  ].join('\n') + '\n',
}

test('retries one audit endpoint 503 and preserves the audit arguments', () => {
  const calls = []
  const results = [serviceUnavailable, { status: 0, stdout: 'found 0 vulnerabilities\n', stderr: '' }]

  runNpmAudit(['--omit=dev', '--audit-level=high'], (command, args, options) => {
    calls.push({ command, args, options })
    return results.shift()
  })

  assert.equal(calls.length, 2)
  assert.equal(calls[0].command, 'npm')
  assert.deepEqual(calls[0].args, ['audit', '--omit=dev', '--audit-level=high'])
  assert.equal(calls[0].options.timeout, 9 * 60 * 1000)
})

test('does not retry a vulnerability failure', () => {
  let calls = 0

  assert.throws(
    () =>
      runNpmAudit(['--audit-level=critical'], () => {
        calls += 1
        return { status: 1, stdout: '1 critical severity vulnerability\n', stderr: '' }
      }),
    /npm audit failed with status 1/,
  )
  assert.equal(calls, 1)
})

test('fails closed after a second audit endpoint 503', () => {
  let calls = 0

  assert.throws(
    () =>
      runNpmAudit([], () => {
        calls += 1
        return serviceUnavailable
      }),
    /npm audit failed with status 1/,
  )
  assert.equal(calls, 2)
})

test('security workflow delegates both npm audits to the retry policy', () => {
  const workflow = fs.readFileSync(
    path.join(repositoryRoot, '.github/workflows/security.yml'),
    'utf8',
  )

  assert.equal((workflow.match(/node scripts\/npm-audit-policy\.js/g) || []).length, 2)
  assert.doesNotMatch(workflow, /run: npm audit/)
})
