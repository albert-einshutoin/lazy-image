#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawn } = require('node:child_process');
const { loadConfig } = require('./lib/config');
const { getAdapter } = require('./lib/adapters');

function argument(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

function commandForTarget(target, category, defaultAdapter) {
  const separator = target.indexOf('::');
  const adapterId = separator === -1 ? defaultAdapter : target.slice(0, separator);
  const adapterTarget = separator === -1 ? target : target.slice(separator + 2);
  if (!adapterTarget) throw new Error(`selected test target is empty: ${target}`);
  return getAdapter(adapterId).commandForTarget(adapterTarget, category);
}

function validateAdapterTargets(tasks) {
  const adapterIds = [...new Set(tasks.map(task => task.adapter))];
  for (const adapterId of adapterIds) {
    getAdapter(adapterId).validateTargets(tasks.filter(task => task.adapter === adapterId));
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
        ...plan.unitTestTargets.map(target => commandForTarget(target, 'unit', config.defaultTestAdapter)),
        ...plan.integrationTestTargets.map(target => commandForTarget(target, 'integration', config.defaultTestAdapter)),
        ...plan.e2eTestTargets.map(target => commandForTarget(target, 'e2e', config.defaultTestAdapter)),
        ...plan.smokeTestTargets.map(target => commandForTarget(target, 'smoke', config.defaultTestAdapter)),
      ];
      const deduplicated = new Map(tasks.map(task => [`${task.command}\0${task.args.join('\0')}`, task]));
      tasks = [...deduplicated.values()];
      if (tasks.length === 0) throw new Error('selective strategy produced zero executable targets');
      validateAdapterTargets(tasks);
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
