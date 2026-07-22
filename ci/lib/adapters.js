'use strict';

const adapters = [
  require('../adapters/javascript'),
  require('../adapters/rust'),
  require('../adapters/python'),
  require('../adapters/go'),
];

function adapterForManifest(file) {
  const basename = file.split('/').at(-1);
  return adapters.find(adapter => adapter.manifests.includes(basename));
}

function getAdapter(id) {
  const adapter = adapters.find(candidate => candidate.id === id);
  if (!adapter) throw new Error(`unknown test adapter: ${id}`);
  return adapter;
}

module.exports = { adapters, adapterForManifest, getAdapter };
