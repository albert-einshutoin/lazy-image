#!/usr/bin/env node
'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { loadConfig } = require('./lib/config');
const { detectChanges } = require('./lib/git');
const { detectProjects, planSelectiveTests } = require('./lib/planner');

function argument(name, fallback) {
  const index = process.argv.indexOf(name);
  return index === -1 ? fallback : process.argv[index + 1];
}

function trackedFiles(root) {
  const result = spawnSync('git', ['ls-files'], { cwd: root, encoding: 'utf8' });
  if (result.status !== 0) throw new Error(`git ls-files failed: ${result.stderr.trim()}`);
  return result.stdout.split('\n').filter(Boolean);
}

function fallbackPlan(baseRevision, headRevision, reason) {
  return {
    strategy: 'full',
    baseRevision,
    headRevision,
    changedFiles: [],
    detectedProjects: [],
    affectedProjects: [],
    affectedModules: [],
    unitTestTargets: [],
    integrationTestTargets: [],
    e2eTestTargets: [],
    smokeTestTargets: [],
    fallback: true,
    fallbackReason: reason,
  };
}

function writeGithubOutputs(file, plan) {
  if (!file) return;
  fs.appendFileSync(file, [
    `strategy=${plan.strategy}`,
    `fallback=${plan.fallback}`,
    `target-count=${[
      plan.unitTestTargets,
      plan.integrationTestTargets,
      plan.e2eTestTargets,
      plan.smokeTestTargets,
    ].reduce((count, targets) => count + targets.length, 0)}`,
  ].join('\n') + '\n');
}

function printLog(plan) {
  const categories = {
    unit: plan.unitTestTargets.length,
    integration: plan.integrationTestTargets.length,
    e2e: plan.e2eTestTargets.length,
    smoke: plan.smokeTestTargets.length,
  };
  console.log(`Base revision: ${plan.baseRevision || 'unavailable'}`);
  console.log(`Head revision: ${plan.headRevision || 'unavailable'}`);
  console.log(`Detected projects: ${plan.detectedProjects.length}`);
  console.log(`Adapters: ${[...new Set(plan.detectedProjects.map(project => project.adapter))].join(', ') || 'unavailable'}`);
  console.log(`Changed files (${plan.changedFiles.length}): ${plan.changedFiles.join(', ') || 'unavailable'}`);
  console.log(`Affected projects: ${plan.affectedProjects.join(', ') || 'none'}`);
  console.log(`Affected modules: ${plan.affectedModules.join(', ') || 'none'}`);
  console.log(`Test strategy: ${plan.strategy}`);
  console.log(`Fallback: ${plan.fallback}`);
  if (plan.fallbackReason) console.log(`Fallback reason: ${plan.fallbackReason}`);
  console.log(`Test categories: ${JSON.stringify(categories)}`);
}

function main() {
  const root = path.resolve(__dirname, '..');
  const requestedBase = argument('--base', process.env.GITHUB_BASE_SHA);
  const requestedHead = argument('--head', process.env.GITHUB_SHA || 'HEAD');
  const output = path.resolve(argument('--output', path.join(root, 'artifacts/ci/test-plan.json')));
  let plan;
  try {
    const config = loadConfig(root);
    const projects = detectProjects(trackedFiles(root), root);
    const diff = detectChanges({ baseRevision: requestedBase, headRevision: requestedHead, cwd: root });
    plan = planSelectiveTests({ config, projects, ...diff });
  } catch (error) {
    // The planner is a safety gate. Any exception becomes an explicit full run;
    // an analysis failure must never turn into a successful test skip.
    plan = fallbackPlan(requestedBase, requestedHead, `impact analysis failed: ${error.message}`);
  }
  fs.mkdirSync(path.dirname(output), { recursive: true });
  fs.writeFileSync(output, `${JSON.stringify(plan, null, 2)}\n`);
  writeGithubOutputs(argument('--github-output', process.env.GITHUB_OUTPUT), plan);
  printLog(plan);
}

main();
