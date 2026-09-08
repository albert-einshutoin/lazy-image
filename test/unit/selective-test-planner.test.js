const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');

const { detectProjects, planSelectiveTests } = require('../../ci/lib/planner');
const { parseNameStatus } = require('../../ci/lib/git');
const { countMatchingRustTests } = require('../../ci/lib/test-targets');
const { loadConfig } = require('../../ci/lib/config');
const { getAdapter } = require('../../ci/lib/adapters');

const fullCiWorkflow = fs.readFileSync(
  path.join(__dirname, '../../.github/workflows/CI.yml'),
  'utf8',
);
assert.match(
  fullCiWorkflow,
  /npm run test:ci-planner:rust-targets/,
  'main/release/scheduled CI must validate configured Rust selectors',
);
assert.match(
  fullCiWorkflow,
  /npm run test:wasm:package/,
  'main/release/scheduled CI must execute Wasm workspace tests',
);

const fixtureConfig = {
  fullTestRules: [
    { pattern: 'Cargo.lock', reason: 'dependency lock file changed' },
    { pattern: '.github/**', reason: 'CI configuration changed' },
  ],
  modules: [
    {
      name: 'memory',
      paths: ['src/engine/memory/**'],
      unitTests: ['memory'],
      integrationTests: ['test/integration/concurrency-validation.test.js'],
    },
    {
      name: 'tasks',
      paths: ['src/engine/tasks/**'],
      dependsOn: ['memory'],
      unitTests: ['tasks'],
      integrationTests: ['test/integration/batch-chunked.test.js'],
    },
    {
      name: 'api',
      paths: ['src/engine/api/**'],
      dependsOn: ['tasks'],
      unitTests: ['api'],
      integrationTests: ['test/integration/basic.test.js'],
    },
    {
      name: 'streaming',
      paths: ['streaming/**'],
      integrationTests: ['test/integration/streaming.test.js'],
    },
  ],
  smokeTests: ['test/integration/basic.test.js'],
  knownSourceRoots: ['src/', 'streaming/'],
};

function change(file, status = 'M', exists = true) {
  return { path: file, status, exists };
}

assert.deepEqual(
  parseNameStatus('M\0src/lib.rs\0D\0old.js\0R100\0before.js\0after.js\0C090\0source.js\0copy.js\0'),
  [
    { path: 'src/lib.rs', status: 'M', exists: true },
    { path: 'old.js', status: 'D', exists: false },
    { path: 'before.js', status: 'D', exists: false },
    { path: 'after.js', oldPath: 'before.js', status: 'R100', exists: true },
    { path: 'copy.js', oldPath: 'source.js', status: 'C090', exists: true },
  ],
  '追加・変更・削除・rename・copyを欠落なく正規化する',
);
assert.throws(
  () => getAdapter('default').commandForTarget('unknown-target'),
  /unsupported adapter target/,
  '未対応形式は実行を推測せず全件フォールバックへ送れる',
);

assert.equal(
  countMatchingRustTests('engine::memory', 'engine::memory::tests::limit: test\nengine::api::tests::smoke: test\n'),
  1,
  'Rustフィルターが実在するテストを選ぶことを事前検証できる',
);

assert.deepEqual(
  getAdapter('python').commandForTarget('tests/test_image.py', 'unit'),
  {
    adapter: 'python',
    category: 'unit',
    target: 'tests/test_image.py',
    command: 'python',
    args: ['-m', 'pytest', 'tests/test_image.py'],
  },
  '言語固有の関連テストコマンドはアダプターが生成する',
);
assert.deepEqual(
  getAdapter('go').commandForTarget('./internal/image', 'unit'),
  {
    adapter: 'go',
    category: 'unit',
    target: './internal/image',
    command: 'go',
    args: ['test', './internal/image'],
  },
);

{
  const root = path.join(__dirname, '../..');
  const config = loadConfig(root);
  for (const module of config.modules) {
    for (const target of module.unitTests || []) {
      if (target.startsWith('javascript::')) {
        assert.ok(fs.existsSync(path.join(root, target.slice(12))), `${target} must exist`);
      }
    }
    for (const target of [...(module.integrationTests || []), ...(module.e2eTests || [])]) {
      assert.ok(fs.existsSync(path.join(root, target)), `${target} must exist`);
    }
  }
}

{
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change('test/type-safety/runtime-type-safety.test.js')],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'selective');
  assert.ok(
    plan.unitTestTargets.includes('javascript::test/type-safety/runtime-type-safety.test.js'),
    '変更されたtype-safetyテスト自体を必ず実行する',
  );
}

assert.deepEqual(
  detectProjects(
    ['package.json', 'Cargo.toml', 'pyproject.toml', 'go.mod'],
    path.join(__dirname, '../..'),
  ).map(project => project.adapter),
  ['javascript', 'rust', 'python', 'go'],
  '複数言語をそれぞれのアダプターとして検出する',
);

{
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change('Cargo.lock')],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'full');
  assert.equal(plan.fallback, true);
  assert.match(plan.fallbackReason, /dependency lock file/);
}

{
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change('src/engine/memory/estimate.rs')],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'selective');
  assert.deepEqual(plan.affectedModules, ['api', 'memory', 'tasks']);
  assert.deepEqual(plan.unitTestTargets, ['api', 'memory', 'tasks']);
  assert.deepEqual(plan.integrationTestTargets, [
    'test/integration/basic.test.js',
    'test/integration/batch-chunked.test.js',
    'test/integration/concurrency-validation.test.js',
  ]);
}

{
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change('test/integration/streaming.test.js')],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'selective');
  assert.ok(plan.integrationTestTargets.includes('test/integration/streaming.test.js'));
  assert.ok(plan.smokeTestTargets.includes('test/integration/basic.test.js'));
}

{
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change('src/engine/memory/removed.rs', 'D', false)],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'selective');
  assert.ok(!plan.unitTestTargets.includes('src/engine/memory/removed.rs'));
}

for (const changedPath of ['src/unclassified.rs', 'src/engine/unknown/new.rs']) {
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change(changedPath)],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'full');
  assert.match(plan.fallbackReason, /unclassified source file/);
}

{
  const plan = planSelectiveTests({
    config: fixtureConfig,
    changes: [change('services/new_service.py')],
    baseRevision: 'base',
    headRevision: 'head',
  });
  assert.equal(plan.strategy, 'full');
  assert.match(plan.fallbackReason, /unclassified file/);
}

console.log('selective-test planner tests passed');
