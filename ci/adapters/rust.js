'use strict';

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
};
