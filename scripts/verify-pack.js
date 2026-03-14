const { execFileSync } = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const requiredPackFiles = [
  'CHANGELOG.md',
  'CONTRIBUTING.md',
  'FUZZING.md',
  'LICENSE',
  'README.ja.md',
  'README.md',
  'SECURITY.md',
  'docs/API.md',
  'docs/ARCHITECTURE.md',
  'docs/PERFORMANCE.md',
  'docs/ROADMAP.md',
  'docs/TROUBLESHOOTING.md',
  'docs/VERSION_HISTORY.md',
  'index.d.ts',
  'index.js',
  'package.json',
  'spec/errors.md',
  'spec/limits.md',
  'spec/metadata.md',
  'spec/pipeline.md',
  'spec/quality.md',
  'spec/resize.md',
  'streaming/pipeline.js',
];

function readPackedFiles() {
  const npmExecPath = process.env.npm_execpath;
  const command = npmExecPath
    ? process.execPath
    : process.platform === 'win32'
      ? 'npm.cmd'
      : 'npm';
  const args = npmExecPath
    ? [npmExecPath, 'pack', '--dry-run', '--json']
    : ['pack', '--dry-run', '--json'];
  const stdout = execFileSync(command, args, {
    cwd: root,
    encoding: 'utf8',
  });
  const [summary] = JSON.parse(stdout);
  return new Set(summary.files.map((entry) => entry.path));
}

function collectRelativeMarkdownLinks(filePath) {
  const markdown = fs.readFileSync(filePath, 'utf8');
  const matches = markdown.matchAll(/\]\((\.\/[^)#?]+)(?:#[^)]+)?\)/g);
  return [...matches].map((match) => match[1].replace(/^\.\//, ''));
}

function ensurePackedFile(packedFiles, relPath, context) {
  if (!packedFiles.has(relPath)) {
    throw new Error(`${context}: missing "${relPath}" from npm package`);
  }
}

function main() {
  const packedFiles = readPackedFiles();

  for (const relPath of requiredPackFiles) {
    ensurePackedFile(packedFiles, relPath, 'required file');
  }

  for (const readmeName of ['README.md', 'README.ja.md']) {
    const readmePath = path.join(root, readmeName);
    for (const relPath of collectRelativeMarkdownLinks(readmePath)) {
      if (relPath.endsWith('/')) {
        continue;
      }
      ensurePackedFile(packedFiles, relPath, `${readmeName} link target`);
    }
  }

  console.log(`Pack contract verified: ${packedFiles.size} files included in npm tarball.`);
}

main();
