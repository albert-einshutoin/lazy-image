'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')

const {
  SECURITY_AUDIT_ISSUE_TITLE,
  upsertSecurityAuditIssue,
} = require('../../scripts/security-audit-notification')

const context = {
  serverUrl: 'https://github.com',
  runId: 12345,
  repo: { owner: 'owner', repo: 'lazy-image' },
}

function test(name, fn) {
  Promise.resolve()
    .then(fn)
    .then(
      () => console.log(`ok - ${name}`),
      (error) => {
        console.error(`not ok - ${name}`)
        console.error(error)
        process.exitCode = 1
      },
    )
}

function createGithub({ issues = [], comments = [], failure } = {}) {
  const calls = []
  const invoke = async (name, payload, result) => {
    calls.push({ name, payload })
    if (failure === name) throw new Error('GitHub API unavailable')
    return { data: result }
  }

  return {
    calls,
    rest: {
      issues: {
        listForRepo: (payload) => invoke('listForRepo', payload, issues),
        create: (payload) => invoke('create', payload, { number: 99 }),
        listComments: (payload) => invoke('listComments', payload, comments),
        createComment: (payload) => invoke('createComment', payload, {}),
      },
    },
  }
}

test('creates the canonical security issue when none is open', async () => {
  const github = createGithub()
  const result = await upsertSecurityAuditIssue({ github, context })

  assert.equal(result.action, 'created')
  const create = github.calls.find((call) => call.name === 'create')
  assert.equal(create.payload.title, SECURITY_AUDIT_ISSUE_TITLE)
  assert.deepEqual(create.payload.labels, ['security', 'priority/P1'])
  assert.match(create.payload.body, /actions\/runs\/12345/)
})

test('updates the canonical issue with a new failing run', async () => {
  const github = createGithub({
    issues: [{ number: 42, title: SECURITY_AUDIT_ISSUE_TITLE, body: 'previous run' }],
  })

  const result = await upsertSecurityAuditIssue({ github, context })

  assert.equal(result.action, 'commented')
  const comment = github.calls.find((call) => call.name === 'createComment')
  assert.equal(comment.payload.issue_number, 42)
  assert.match(comment.payload.body, /actions\/runs\/12345/)
})

test('does not duplicate a notification for the same workflow run', async () => {
  const runUrl = 'https://github.com/owner/lazy-image/actions/runs/12345'
  const github = createGithub({
    issues: [{ number: 42, title: SECURITY_AUDIT_ISSUE_TITLE, body: 'previous run' }],
    comments: [{ body: `Already reported: ${runUrl}` }],
  })

  const result = await upsertSecurityAuditIssue({ github, context })

  assert.equal(result.action, 'already-recorded')
  assert.equal(github.calls.some((call) => call.name === 'createComment'), false)
})

test('does not reuse an unrelated open security issue', async () => {
  const github = createGithub({
    issues: [{ number: 7, title: 'Unrelated advisory', body: 'security work' }],
  })

  const result = await upsertSecurityAuditIssue({ github, context })

  assert.equal(result.action, 'created')
})

test('reports notification failures with actionable context', async () => {
  const github = createGithub({ failure: 'listForRepo' })

  await assert.rejects(
    upsertSecurityAuditIssue({ github, context }),
    /Security audit notification failed: GitHub API unavailable/,
  )
})

test('only the Cargo audit job receives issue write access and calls the tested module', () => {
  const workflow = fs.readFileSync(
    path.join(__dirname, '../../.github/workflows/security.yml'),
    'utf8',
  )

  assert.match(workflow, /permissions:\n  contents: read\n\njobs:/)
  assert.match(
    workflow,
    /audit:\n    name: Cargo Audit\n    runs-on: ubuntu-latest\n    permissions:\n      contents: read\n      issues: write/,
  )
  assert.equal((workflow.match(/issues: write/g) || []).length, 1)
  assert.match(workflow, /scripts\/security-audit-notification\.js/)
})
