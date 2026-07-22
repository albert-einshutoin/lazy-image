'use strict';

const { spawnSync } = require('node:child_process');
const { countMatchingRustTests } = require('../lib/test-targets');

module.exports = {
  id: 'rust',
  manifests: ['Cargo.toml'],
  fullTestCommands: [['cargo', ['test', '--no-default-features']]],
  buildCommands: [['cargo', ['check', '--no-default-features']]],
  staticCommands: [
    ['cargo', ['fmt', '--check']],
    ['cargo', ['clippy', '--no-default-features', '--', '-D', 'warnings']],
  ],
  cachePaths: ['~/.cargo/registry', 'target'],
  riskPatterns: ['Cargo.toml', 'Cargo.lock', 'build.rs', 'rust-toolchain*'],
  relatedTestSelection: 'cargo test name filter validated against cargo test -- --list',
  commandForTarget(target, category) {
    return {
      adapter: 'rust',
      category,
      target,
      command: 'cargo',
      args: ['test', '--no-default-features', target],
    };
  },
  validateTargets(tasks) {
    if (tasks.length === 0) return;
    const result = spawnSync('cargo', ['test', '--no-default-features', '--', '--list'], {
      encoding: 'utf8',
      maxBuffer: 16 * 1024 * 1024,
    });
    if (result.status !== 0) {
      throw new Error(`Rust test discovery failed: ${(result.stderr || result.stdout).trim()}`);
    }
    for (const task of tasks) {
      if (countMatchingRustTests(task.target, result.stdout) === 0) {
        throw new Error(`Rust test filter selected zero tests: ${task.target}`);
      }
    }
  },
};
