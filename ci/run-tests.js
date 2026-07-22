#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawn, spawnSync } = require('node:child_process');
const { loadConfig } = require('./lib/config');
const { countMatchingRustTests } = require('./lib/test-targets');

function argument(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

function commandForTarget(target, category) {
  if (target.startsWith('rust::')) {
    return { category, target, command: 'cargo', args: ['test', '--no-default-features', target.slice(6)] };
  }
  const file = target.startsWith('javascript::') ? target.slice(12) : target;
  if (!file || !fs.existsSync(file)) throw new Error(`selected test target does not exist: ${file || target}`);
  return { category, target: file, command: process.execPath, args: [file] };
}

function validateRustTargets(tasks) {
  const filters = tasks
    .filter(task => task.command === 'cargo')
    .map(task => task.args.at(-1));
  if (filters.length === 0) return;
  const result = spawnSync('cargo', ['test', '--no-default-features', '--', '--list'], {
    encoding: 'utf8',
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.status !== 0) {
    throw new Error(`Rust test discovery failed: ${(result.stderr || result.stdout).trim()}`);
  }
  for (const filter of filters) {
    if (countMatchingRustTests(filter, result.stdout) === 0) {
      throw new Error(`Rust test filter selected zero tests: ${filter}`);
    }
  }
}

function execute(task) {
  const started = Date.now();
  console.log(`\n[${task.category}] ${task.command} ${task.args.join(' ')}`);
  return new Promise(resolve => {
    const child = spawn(task.command, task.args, { stdio: 'inherit', env: process.env });
    child.on('error', error => resolve({ ...task, success: false, error: error.message, durationMs: Date.now() - started }));
    child.on('exit', code => resolve({ ...task, success: code === 0, exitCode: code, durationMs: Date.now() - started }));
  });
}

async function runPool(tasks, maximum) {
  const queue = [...tasks];
  const results = [];
  async function worker() {
    while (queue.length > 0) results.push(await execute(queue.shift()));
  }
  await Promise.all(Array.from({ length: Math.min(maximum, tasks.length) }, worker));
  return results;
}

function countAllTests(root) {
  const stack = [path.join(root, 'test')];
  let count = 0;
  while (stack.length > 0) {
    const current = stack.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const location = path.join(current, entry.name);
      if (entry.isDirectory()) stack.push(location);
      else if (/\.(test|spec)\.(js|mjs|ts)$/.test(entry.name)) count += 1;
    }
  }
  return count;
}

async function main() {
  const root = path.resolve(__dirname, '..');
  process.chdir(root);
  const planPath = path.resolve(argument('--plan', 'artifacts/ci/test-plan.json'));
  const summaryPath = path.resolve(argument('--summary', 'artifacts/ci/test-summary.json'));
  const historyPath = path.resolve(argument('--history', 'artifacts/ci/test-history.jsonl'));
  const config = loadConfig(root);
  const plan = JSON.parse(fs.readFileSync(planPath, 'utf8'));
  const forcedMode = argument('--mode');
  const mode = forcedMode || plan.strategy;
  const started = Date.now();
  let tasks;
  let effectiveMode = mode;
  let runtimeFallbackReason = null;

  try {
    if (mode === 'full') {
      const [command, ...args] = config.fullTestCommand;
      tasks = [{ category: 'full', target: 'all', command, args }];
    } else {
      tasks = [
        ...plan.unitTestTargets.map(target => commandForTarget(target, 'unit')),
        ...plan.integrationTestTargets.map(target => commandForTarget(target, 'integration')),
        ...plan.e2eTestTargets.map(target => commandForTarget(target, 'e2e')),
        ...plan.smokeTestTargets.map(target => commandForTarget(target, 'smoke')),
      ];
      const deduplicated = new Map(tasks.map(task => [`${task.command}\0${task.args.join('\0')}`, task]));
      tasks = [...deduplicated.values()];
      if (tasks.length === 0) throw new Error('selective strategy produced zero executable targets');
      validateRustTargets(tasks);
    }
  } catch (error) {
    effectiveMode = 'full';
    runtimeFallbackReason = `test selection failed: ${error.message}`;
    console.error(`${runtimeFallbackReason}; running the full suite`);
    const [command, ...args] = config.fullTestCommand;
    tasks = [{ category: 'full', target: 'all', command, args }];
  }

  const results = await runPool(tasks, Number(config.maxParallelTests) || 1);
  const summary = {
    requestedStrategy: mode,
    strategy: effectiveMode,
    fallback: plan.fallback || Boolean(runtimeFallbackReason),
    fallbackReason: runtimeFallbackReason || plan.fallbackReason,
    selectedTestTargets: tasks.length,
    allTestFiles: countAllTests(root),
    successCount: results.filter(result => result.success).length,
    failureCount: results.filter(result => !result.success).length,
    skippedCount: 0,
    wallDurationMs: Date.now() - started,
    totalComputeMs: results.reduce((total, result) => total + result.durationMs, 0),
    commit: process.env.GITHUB_SHA || null,
    branch: process.env.GITHUB_HEAD_REF || process.env.GITHUB_REF_NAME || null,
    results,
  };
  fs.mkdirSync(path.dirname(summaryPath), { recursive: true });
  fs.writeFileSync(summaryPath, `${JSON.stringify(summary, null, 2)}\n`);
  fs.appendFileSync(historyPath, `${JSON.stringify({ ...summary, results: undefined })}\n`);
  console.log(`\nTest strategy: ${summary.strategy}`);
  console.log(`Targets: ${summary.selectedTestTargets}/${summary.allTestFiles}`);
  console.log(`Success: ${summary.successCount}, failure: ${summary.failureCount}, skipped: ${summary.skippedCount}`);
  console.log(`Wall time: ${summary.wallDurationMs} ms, total compute: ${summary.totalComputeMs} ms`);
  if (summary.failureCount > 0) process.exitCode = 1;
}

main().catch(error => {
  console.error(`test runner failed before execution: ${error.stack || error.message}`);
  process.exitCode = 1;
});
