'use strict';

const fs = require('node:fs');
const path = require('node:path');

function readJson(root, file) {
  const location = path.join(root, 'ci/config', file);
  return JSON.parse(fs.readFileSync(location, 'utf8'));
}

function loadConfig(root) {
  const settings = readJson(root, 'project-settings.json');
  const config = {
    ...settings,
    fullTestRules: readJson(root, 'full-test-rules.json'),
    modules: readJson(root, 'module-mappings.json'),
    smokeTests: readJson(root, 'smoke-tests.json'),
  };
  if (!Array.isArray(config.fullTestRules) || !Array.isArray(config.modules)
      || !Array.isArray(config.smokeTests)) {
    throw new Error('selective-test configuration arrays are invalid');
  }
  return config;
}

module.exports = { loadConfig };
