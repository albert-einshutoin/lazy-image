'use strict';

module.exports = {
  id: 'default',
  manifests: [],
  fullTestCommands: [],
  buildCommands: [],
  staticCommands: [],
  cachePaths: [],
  riskPatterns: ['**'],
  relatedTestSelection: 'unsupported; always use full fallback',
  commandForTarget(target) {
    throw new Error(`unsupported adapter target: ${target}`);
  },
  validateTargets() {},
};
