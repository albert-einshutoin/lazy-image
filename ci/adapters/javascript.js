'use strict';

const fs = require('node:fs');

module.exports = {
  id: 'javascript',
  manifests: ['package.json'],
  fullTestCommands: [['npm', ['test']]],
  buildCommands: [['npm', ['run', 'build']]],
  staticCommands: [['npm', ['run', 'test:types']]],
  cachePaths: ['~/.npm'],
  riskPatterns: ['package.json', 'package-lock.json', 'tsconfig*.json'],
  relatedTestSelection: 'explicit test path or framework-provided related-test target',
  commandForTarget(target, category) {
    return { adapter: 'javascript', category, target, command: process.execPath, args: [target] };
  },
  validateTargets(tasks) {
    for (const task of tasks) {
      if (!fs.existsSync(task.target)) throw new Error(`selected JavaScript test does not exist: ${task.target}`);
    }
  },
};
