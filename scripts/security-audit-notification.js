'use strict'

const SECURITY_AUDIT_ISSUE_TITLE = '🚨 Security vulnerability detected in dependencies'
const SECURITY_AUDIT_LABELS = ['security', 'priority/P1']

function workflowRunUrl(context) {
  const { serverUrl, runId, repo } = context || {}
  if (!serverUrl || !runId || !repo?.owner || !repo?.repo) {
    throw new Error('workflow context is missing serverUrl, runId, or repository details')
  }
  return `${serverUrl}/${repo.owner}/${repo.repo}/actions/runs/${runId}`
}

function notificationBody(runUrl, existingIssue = false) {
  const heading = existingIssue
    ? 'Cargo audit failed again on the scheduled security scan.'
    : 'Cargo audit found a dependency vulnerability or policy violation.'

  return [
    heading,
    '',
    `Workflow run: ${runUrl}`,
    '',
    'Review the advisory output, identify whether the dependency is direct or transitive,',
    'and either upgrade it or document a time-bounded exception with the upstream tracking issue.',
  ].join('\n')
}

async function upsertSecurityAuditIssue({ github, context }) {
  try {
    const runUrl = workflowRunUrl(context)
    const repository = { owner: context.repo.owner, repo: context.repo.repo }
    const response = await github.rest.issues.listForRepo({
      ...repository,
      labels: 'security',
      state: 'open',
      per_page: 100,
    })

    // Match the canonical title instead of reusing any security issue: an unrelated
    // advisory must not hide a new audit failure or receive noisy weekly comments.
    const issue = response.data.find(({ title }) => title === SECURITY_AUDIT_ISSUE_TITLE)
    if (!issue) {
      const created = await github.rest.issues.create({
        ...repository,
        title: SECURITY_AUDIT_ISSUE_TITLE,
        body: notificationBody(runUrl),
        labels: SECURITY_AUDIT_LABELS,
      })
      return { action: 'created', issueNumber: created.data.number }
    }

    // A re-run can execute the failure step more than once. Persisting the run URL
    // gives us an idempotency key without maintaining external state.
    if (issue.body?.includes(runUrl)) {
      return { action: 'already-recorded', issueNumber: issue.number }
    }
    const comments = await github.rest.issues.listComments({
      ...repository,
      issue_number: issue.number,
      per_page: 100,
    })
    if (comments.data.some(({ body }) => body?.includes(runUrl))) {
      return { action: 'already-recorded', issueNumber: issue.number }
    }

    await github.rest.issues.createComment({
      ...repository,
      issue_number: issue.number,
      body: notificationBody(runUrl, true),
    })
    return { action: 'commented', issueNumber: issue.number }
  } catch (error) {
    throw new Error(`Security audit notification failed: ${error.message}`, { cause: error })
  }
}

module.exports = {
  SECURITY_AUDIT_ISSUE_TITLE,
  upsertSecurityAuditIssue,
}
