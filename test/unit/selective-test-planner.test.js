const assert = require('node:assert/strict');
const path = require('node:path');

const { detectProjects, planSelectiveTests } = require('../../ci/lib/planner');
const { parseNameStatus } = require('../../ci/lib/git');

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
