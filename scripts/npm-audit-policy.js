'use strict'

const { spawnSync } = require('node:child_process')

const MAX_ATTEMPTS = 2
const AUDIT_TIMEOUT_MS = 9 * 60 * 1000

function isRetryableAuditFailure(result) {
  const output = `${result.stdout || ''}\n${result.stderr || ''}`
  return (
    output.includes('503 Service Unavailable') &&
    output.includes('audit endpoint returned an error')
  )
}

function runNpmAudit(args, runCommand = spawnSync) {
  for (let attempt = 1; attempt <= MAX_ATTEMPTS; attempt += 1) {
    const result = runCommand('npm', ['audit', ...args], {
      encoding: 'utf8',
      timeout: AUDIT_TIMEOUT_MS,
    })

    if (result.stdout) process.stdout.write(result.stdout)
    if (result.stderr) process.stderr.write(result.stderr)
    if (result.error) throw result.error
    if (result.status === 0) return

    if (attempt < MAX_ATTEMPTS && isRetryableAuditFailure(result)) {
      console.error('npm audit endpoint returned 503; retrying once.')
      continue
    }

    throw new Error(`npm audit failed with status ${result.status}`)
  }
}

if (require.main === module) {
  try {
    runNpmAudit(process.argv.slice(2))
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}

module.exports = { isRetryableAuditFailure, runNpmAudit }
