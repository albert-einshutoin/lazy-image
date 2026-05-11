/**
 * Verifies that the runtime type-safety test exits non-zero on unexpected
 * rejection so CI catches regressions.
 */

const assert = require('assert');
const { spawnSync } = require('child_process');
const { resolveRoot } = require('../helpers/paths');

const result = spawnSync(
    process.execPath,
    [resolveRoot('test/type-safety/runtime-type-safety.test.js')],
    {
        env: {
            ...process.env,
            LAZY_IMAGE_FORCE_RUNTIME_TYPE_SAFETY_FAILURE: '1',
        },
        encoding: 'utf8',
    },
);

assert.notStrictEqual(
    result.status,
    0,
    `forced runtime type-safety failure should exit non-zero; stdout=${result.stdout} stderr=${result.stderr}`,
);

console.log('runtime-type-safety-exit.test.js passed');
