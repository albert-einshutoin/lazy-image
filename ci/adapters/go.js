'use strict';

module.exports = {
  id: 'go',
  manifests: ['go.mod'],
  fullTestCommands: [['go', ['test', './...']]],
  buildCommands: [['go', ['build', './...']]],
  staticCommands: [['go', ['vet', './...']]],
  cachePaths: ['~/.cache/go-build', '~/go/pkg/mod'],
};
