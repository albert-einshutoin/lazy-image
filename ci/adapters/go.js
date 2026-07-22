'use strict';

const { spawnSync } = require('node:child_process');

module.exports = {
  id: 'go',
  manifests: ['go.mod'],
  fullTestCommands: [['go', ['test', './...']]],
  buildCommands: [['go', ['build', './...']]],
  staticCommands: [['go', ['vet', './...']]],
  cachePaths: ['~/.cache/go-build', '~/go/pkg/mod'],
  riskPatterns: ['go.mod', 'go.sum'],
  relatedTestSelection: 'go test package target validated with go list',
  commandForTarget(target, category) {
    return { adapter: 'go', category, target, command: 'go', args: ['test', target] };
  },
  validateTargets(tasks) {
    for (const task of tasks) {
      const result = spawnSync('go', ['list', task.target], { encoding: 'utf8' });
      if (result.status !== 0) {
        throw new Error(`Go test target cannot be resolved: ${task.target}`);
      }
    }
  },
};
