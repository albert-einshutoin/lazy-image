const path = require('path');

// リポジトリルートへのパスを共通で扱うヘルパー
const ROOT_DIR = path.resolve(__dirname, '..', '..');
const TEST_DIR = path.join(ROOT_DIR, 'test');
const FIXTURES_DIR = path.join(TEST_DIR, 'fixtures');
const TEMP_DIR = path.join(ROOT_DIR, '.tmp');

const resolveRoot = (...paths) => path.join(ROOT_DIR, ...paths);
const resolveFixture = (...paths) => path.join(FIXTURES_DIR, ...paths);
const resolveTemp = (...paths) => path.join(TEMP_DIR, ...paths);

module.exports = {
  ROOT_DIR,
  TEST_DIR,
  FIXTURES_DIR,
  TEMP_DIR,
  resolveRoot,
  resolveFixture,
  resolveTemp,
};
