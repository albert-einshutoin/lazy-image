'use strict'

const fs = require('node:fs')
const path = require('node:path')
const { spawnSync } = require('node:child_process')

const POLICY_PATH = '.github/security/cargo-audit-exceptions.json'
const ADVISORY_PATTERN = /^RUSTSEC-\d{4}-\d{4}$/
const REVIEW_DATE_PATTERN = /^\d{4}-\d{2}-\d{2}$/
const AUDITED_LOCKFILES = ['Cargo.lock', 'fuzz/Cargo.lock']

function isValidReviewDate(reviewBy) {
  if (!REVIEW_DATE_PATTERN.test(reviewBy || '')) return false

  // Round-trip through UTC so JavaScript's permissive date normalization cannot
  // turn an invalid policy deadline such as 2026-13-40 into a different date.
  const parsed = new Date(`${reviewBy}T00:00:00Z`)
  return !Number.isNaN(parsed.valueOf()) && parsed.toISOString().slice(0, 10) === reviewBy
}

function validateAuditPolicy(policy, now = new Date()) {
  if (!policy || !Array.isArray(policy.exceptions)) {
    throw new Error('Cargo audit policy must contain an exceptions array')
  }

  const today = now.toISOString().slice(0, 10)
  const advisoryIds = new Set()

  for (const exception of policy.exceptions) {
    if (!ADVISORY_PATTERN.test(exception.advisory || '')) {
      throw new Error(`Invalid advisory ID: ${exception.advisory || '<missing>'}`)
    }
    if (advisoryIds.has(exception.advisory)) {
      throw new Error(`Duplicate advisory exception: ${exception.advisory}`)
    }
    advisoryIds.add(exception.advisory)

    for (const field of ['package', 'reason', 'impact', 'remediation', 'owner']) {
      if (typeof exception[field] !== 'string' || exception[field].trim() === '') {
        throw new Error(`${exception.advisory} must define ${field}`)
      }
    }

    if (
      !Array.isArray(exception.lockfiles) ||
      exception.lockfiles.length === 0 ||
      exception.lockfiles.some((lockfile) => !AUDITED_LOCKFILES.includes(lockfile)) ||
      new Set(exception.lockfiles).size !== exception.lockfiles.length
    ) {
      throw new Error(
        `${exception.advisory} must define unique lockfiles from: ${AUDITED_LOCKFILES.join(', ')}`,
      )
    }

    try {
      const upstream = new URL(exception.upstream)
      if (upstream.protocol !== 'https:') throw new Error('HTTPS required')
    } catch {
      throw new Error(`${exception.advisory} must define a valid HTTPS upstream URL`)
    }

    if (!isValidReviewDate(exception.reviewBy)) {
      throw new Error(`${exception.advisory} must define reviewBy as a valid YYYY-MM-DD date`)
    }
    if (exception.reviewBy < today) {
      throw new Error(
        `${exception.advisory} exception expired on ${exception.reviewBy}; review or remove it`,
      )
    }
  }

  return policy
}

function loadAuditPolicy(repositoryRoot = path.join(__dirname, '..'), now = new Date()) {
  const policyPath = path.join(repositoryRoot, POLICY_PATH)
  const policy = JSON.parse(fs.readFileSync(policyPath, 'utf8'))
  return validateAuditPolicy(policy, now)
}

function buildAuditCommands(policy, now = new Date()) {
  const validatedPolicy = validateAuditPolicy(policy, now)
  const denyArgs = ['--deny', 'unmaintained', '--deny', 'unsound']

  // Audit both lockfiles explicitly because the fuzz crate has an independent
  // dependency graph and stale pins there must not be hidden by a clean root audit.
  return AUDITED_LOCKFILES.map((lockfile) => {
    // Scope exceptions to the lockfile whose dependency path was reviewed. A
    // new occurrence in the other graph must fail CI and receive its own assessment.
    const ignoreArgs = validatedPolicy.exceptions
      .filter((exception) => exception.lockfiles.includes(lockfile))
      .flatMap(({ advisory }) => ['--ignore', advisory])
    const fileArgs = lockfile === 'Cargo.lock' ? [] : ['--file', lockfile]
    return {
      label: lockfile,
      args: ['audit', ...fileArgs, ...denyArgs, ...ignoreArgs],
    }
  })
}

function runAuditPolicy(repositoryRoot = path.join(__dirname, '..')) {
  const policy = loadAuditPolicy(repositoryRoot)

  for (const command of buildAuditCommands(policy)) {
    console.log(`Auditing ${command.label}`)
    const result = spawnSync('cargo', command.args, {
      cwd: repositoryRoot,
      stdio: 'inherit',
    })
    if (result.error) throw result.error
    if (result.status !== 0) {
      throw new Error(`cargo audit failed for ${command.label} with status ${result.status}`)
    }
  }
}

if (require.main === module) {
  try {
    runAuditPolicy()
  } catch (error) {
    console.error(error.message)
    process.exitCode = 1
  }
}

module.exports = {
  buildAuditCommands,
  loadAuditPolicy,
  runAuditPolicy,
  validateAuditPolicy,
}
