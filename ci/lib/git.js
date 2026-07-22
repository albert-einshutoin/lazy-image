'use strict';

const { spawnSync } = require('node:child_process');

function git(args, options = {}) {
  const result = spawnSync('git', args, {
    cwd: options.cwd,
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(' ')} failed: ${(result.stderr || result.stdout).trim()}`);
  }
  return result.stdout;
}

function parseNameStatus(output) {
  const fields = output.split('\0');
  if (fields.at(-1) === '') fields.pop();
  const changes = [];
  for (let index = 0; index < fields.length;) {
    const status = fields[index++];
    if (!status) throw new Error('empty change status in git diff output');
    if (status.startsWith('R')) {
      const oldPath = fields[index++];
      const newPath = fields[index++];
      if (!oldPath || !newPath) throw new Error(`incomplete rename record: ${status}`);
      changes.push({ path: oldPath, status: 'D', exists: false });
      changes.push({ path: newPath, oldPath, status, exists: true });
    } else if (status.startsWith('C')) {
      const oldPath = fields[index++];
      const newPath = fields[index++];
      if (!oldPath || !newPath) throw new Error(`incomplete copy record: ${status}`);
      changes.push({ path: newPath, oldPath, status, exists: true });
    } else {
      const file = fields[index++];
      if (!file) throw new Error(`missing path for change status: ${status}`);
      changes.push({ path: file, status, exists: status !== 'D' });
    }
  }
  return changes;
}

function hasRevision(revision, cwd) {
  const result = spawnSync('git', ['cat-file', '-e', `${revision}^{commit}`], { cwd, stdio: 'ignore' });
  return result.status === 0;
}

function ensureRevision(revision, cwd) {
  if (hasRevision(revision, cwd)) return;
  const attempts = [
    ['fetch', '--no-tags', '--depth=200', 'origin', revision],
    ['fetch', '--no-tags', '--unshallow', 'origin'],
  ];
  for (const args of attempts) {
    spawnSync('git', args, { cwd, stdio: 'ignore' });
    if (hasRevision(revision, cwd)) return;
  }
  throw new Error(`base revision is unavailable after fetch attempts: ${revision}`);
}

function detectChanges({ baseRevision, headRevision = 'HEAD', cwd }) {
  if (!baseRevision) throw new Error('base revision is required');
  ensureRevision(baseRevision, cwd);
  ensureRevision(headRevision, cwd);
  const mergeBase = git(['merge-base', baseRevision, headRevision], { cwd }).trim();
  if (!mergeBase) throw new Error('git merge-base returned an empty revision');
  const output = git([
    'diff', '--name-status', '-z', '--find-renames', '--find-copies', `${mergeBase}..${headRevision}`,
  ], { cwd });
  return { baseRevision: mergeBase, headRevision, changes: parseNameStatus(output) };
}

module.exports = { detectChanges, parseNameStatus };
