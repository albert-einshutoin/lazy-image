'use strict';

module.exports = {
  id: 'javascript',
  manifests: ['package.json'],
  fullTestCommands: [['npm', ['test']]],
  buildCommands: [['npm', ['run', 'build']]],
  staticCommands: [['npm', ['run', 'test:types']]],
  cachePaths: ['~/.npm'],
};
