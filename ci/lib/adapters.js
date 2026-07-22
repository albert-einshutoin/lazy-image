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

module.exports = { adapters, adapterForManifest };
