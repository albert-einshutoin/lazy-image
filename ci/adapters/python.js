'use strict';

module.exports = {
  id: 'python',
  manifests: ['pyproject.toml', 'requirements.txt', 'setup.py'],
  fullTestCommands: [['python', ['-m', 'pytest']]],
  buildCommands: [['python', ['-m', 'build']]],
  staticCommands: [],
  cachePaths: ['~/.cache/pip'],
};
