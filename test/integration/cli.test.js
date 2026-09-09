const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');

const { compileImage } = require('../../index');
const { resolveFixture, ROOT_DIR } = require('../helpers/paths');

const CLI = path.join(ROOT_DIR, 'bin', 'lazy-image.js');
const INPUT = resolveFixture('test_100KB_1057x1057.jpg');

function runCli(args) {
  return spawnSync(process.execPath, [CLI, ...args], {
    cwd: ROOT_DIR,
    encoding: 'utf8',
    timeout: 120_000,
  });
}

async function main() {
  const parent = await fsp.mkdtemp(path.join(os.tmpdir(), 'lazy-image-cli-'));
  try {
    const policyPath = path.join(parent, 'policy.json');
    await fsp.writeFile(policyPath, JSON.stringify({
      widths: [320, 640],
      formats: ['webp'],
      placeholder: false,
    }));

    const cliOutputDir = path.join(parent, 'cli-output');
    const cliResult = runCli([
      'compile', INPUT, '--out-dir', cliOutputDir, '--policy', policyPath,
    ]);
    assert.equal(cliResult.status, 0, cliResult.stderr);
    assert.equal(cliResult.stderr, '');
    const cliManifest = JSON.parse(cliResult.stdout);
    assert.equal(cliManifest.artifacts.length, 2);
    assert.equal(
      await fsp.stat(path.join(cliOutputDir, 'manifest.json')).then(() => true, () => false),
      true,
    );

    const apiManifest = await compileImage({
      inputPath: INPUT,
      outputDir: path.join(parent, 'api-output'),
      policy: { widths: [320, 640], formats: ['webp'], placeholder: false },
    });
    assert.deepEqual(cliManifest, apiManifest);

    const unknownFlag = runCli([
      'compile', INPUT, '--out-dir', path.join(parent, 'unknown'), '--policy', policyPath, '--nope',
    ]);
    assert.equal(unknownFlag.status, 2);
    assert.equal(unknownFlag.stdout, '');
    assert.match(unknownFlag.stderr, /unknown option/i);

    const extraPositional = runCli([
      'compile', INPUT, 'extra', '--out-dir', path.join(parent, 'extra'), '--policy', policyPath,
    ]);
    assert.equal(extraPositional.status, 2);
    assert.equal(extraPositional.stdout, '');

    const invalidJsonPath = path.join(parent, 'invalid.json');
    await fsp.writeFile(invalidJsonPath, '{');
    const invalidJsonOutput = path.join(parent, 'invalid-json-output');
    const invalidJson = runCli([
      'compile', INPUT, '--out-dir', invalidJsonOutput, '--policy', invalidJsonPath,
    ]);
    assert.equal(invalidJson.status, 2);
    assert.equal(invalidJson.stdout, '');
    assert.match(invalidJson.stderr, /JSON is invalid/i);
    assert.equal(await fsp.stat(invalidJsonOutput).catch(() => null), null);

    const invalidPolicyPath = path.join(parent, 'invalid-policy.json');
    await fsp.writeFile(invalidPolicyPath, JSON.stringify({ widths: [], formats: ['webp'] }));
    const invalidPolicyOutput = path.join(parent, 'invalid-policy-output');
    const invalidPolicy = runCli([
      'compile', INPUT, '--out-dir', invalidPolicyOutput, '--policy', invalidPolicyPath,
    ]);
    assert.equal(invalidPolicy.status, 1, invalidPolicy.stderr);
    assert.equal(invalidPolicy.stdout, '');
    assert.match(invalidPolicy.stderr, /E400/);
    assert.equal(await fsp.stat(invalidPolicyOutput).catch(() => null), null);

    const existingOutput = path.join(parent, 'existing-output');
    await fsp.mkdir(existingOutput);
    const existing = runCli([
      'compile', INPUT, '--out-dir', existingOutput, '--policy', policyPath,
    ]);
    assert.equal(existing.status, 1, existing.stderr);
    assert.equal(existing.stdout, '');
    assert.match(existing.stderr, /E301/);
    assert.deepEqual(await fsp.readdir(existingOutput), []);
  } finally {
    await fsp.rm(parent, { recursive: true, force: true });
  }
}

main().then(
  () => console.log('CLI contract passed'),
  (error) => {
    console.error(error);
    process.exitCode = 1;
  },
);
