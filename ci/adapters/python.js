'use strict';

const fs = require('node:fs');

module.exports = {
  id: 'python',
  manifests: ['pyproject.toml', 'requirements.txt', 'setup.py'],
  fullTestCommands: [['python', ['-m', 'pytest']]],
  buildCommands: [['python', ['-m', 'build']]],
  staticCommands: [],
  cachePaths: ['~/.cache/pip'],
  riskPatterns: ['pyproject.toml', 'requirements*.txt', 'setup.py', 'setup.cfg'],
  relatedTestSelection: 'pytest path, node id, or framework-provided related-test target',
  commandForTarget(target, category) {
    return { adapter: 'python', category, target, command: 'python', args: ['-m', 'pytest', target] };
  },
  validateTargets(tasks) {
    for (const task of tasks) {
      const file = task.target.split('::')[0];
      if (!fs.existsSync(file)) throw new Error(`selected Python test does not exist: ${file}`);
    }
  },
};
