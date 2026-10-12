'use strict';
const assert = require('node:assert/strict');
const { assertBudgetedQuality } = require('../../scripts/registry-native-smoke.cjs');
const budget = { maxBytes: 1024 * 1024, maxQuality: 30 };
const selected = { quality: 30, bytes: 123, targetBytes: budget.maxBytes };
assert.equal(typeof assertBudgetedQuality, 'function', 'the installed-package smoke must check the quality ceiling');
assert.doesNotThrow(() => assertBudgetedQuality(selected, budget));
assert.throws(() => assertBudgetedQuality({ ...selected, quality: 100 }, budget), /quality ceiling/, 'ignoring maxQuality must fail even when bytes fit');
for (const quality of [29, 31, 30.5, '30', null, undefined]) {
  assert.throws(() => assertBudgetedQuality({ ...selected, quality }, budget));
}
assert.throws(() => assertBudgetedQuality({ ...selected, bytes: budget.maxBytes + 1 }, budget));
assert.throws(() => assertBudgetedQuality({ ...selected, targetBytes: budget.maxBytes - 1 }, budget));
console.log('installed-package maxQuality smoke rejects ignored ceilings and invalid artifact facts');
