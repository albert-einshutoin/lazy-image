const assert = require('node:assert/strict');
const { spawnSync, fork, execFileSync } = require('node:child_process');
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

    for (const [name, input, policy, code] of [
      ['budget', INPUT, { widths: [320], formats: ['webp'], budgets: [{ width: 320, format: 'webp', maxBytes: 1 }] }, 'E300'],
      ['bad-input', invalidJsonPath, { widths: [320], formats: ['webp'] }, null],
    ]) {
      const output = path.join(parent, name);
      await fsp.writeFile(policyPath, JSON.stringify(policy));
      const result = runCli(['compile', input, '--out-dir', output, '--policy', policyPath]);
      assert.equal(result.status, 1, result.stderr);
      assert.equal(result.stdout, '');
      assert.ok(result.stderr.length > 0);
      if (code) assert.ok(result.stderr.includes(code), result.stderr);
      assert.equal(await fsp.stat(output).catch(() => null), null);
      assert.deepEqual((await fsp.readdir(parent)).filter(name => name.includes('staging')), []);
    }

    await fsp.writeFile(policyPath, JSON.stringify({ widths: [320], formats: ['webp'] }));
    for (const signal of ['SIGINT', 'SIGTERM']) {
      const output = path.join(parent, signal);
      const child = fork(CLI, ['compile', INPUT, '--out-dir', output, '--policy', policyPath], {
        execArgv: ['--require', path.join(__dirname, '../helpers/cli-signal.cjs')],
        silent: true,
      });
      let stdout = '';
      let stderr = '';
      let ready = false;
      child.stdout.on('data', chunk => { stdout += chunk; });
      child.stderr.on('data', chunk => { stderr += chunk; });
      child.on('message', message => {
        if (message !== 'staged') return;
        ready = true;
        // Windows kill() forcibly terminates; deliver the same process event via IPC there.
        if (process.platform === 'win32') child.send(signal);
        else child.kill(signal);
      });
      const timer = setTimeout(() => child.kill('SIGKILL'), 30_000);
      try {
        const result = await new Promise((resolve, reject) => {
          child.once('error', reject);
          child.once('close', (code, signal) => resolve({ code, signal }));
        });
        assert.ok(ready, stderr);
        assert.deepEqual(result, { code: 1, signal: null }, stderr);
        assert.equal(stdout, '');
        assert.match(stderr, /ABORT_ERR/);
        assert.equal(await fsp.stat(output).catch(() => null), null);
        assert.deepEqual((await fsp.readdir(parent)).filter(name => name.includes('staging')), []);
      } finally {
        clearTimeout(timer);
      }
    }

    const npm = process.env.npm_execpath;
    assert.ok(npm, 'Run through npm run test:cli so the npm CLI path is explicit.');
    const runNpm = (args, cwd) => execFileSync(process.execPath, [npm, ...args], { cwd, encoding: 'utf8' });
    const [packed] = JSON.parse(runNpm(['pack', '--json', '--pack-destination', parent], ROOT_DIR));
    const installDir = path.join(parent, 'installed');
    await fsp.mkdir(installDir);
    runNpm(['install', '--ignore-scripts', '--omit=optional', '--no-audit', '--no-fund', path.join(parent, packed.filename)], installDir);
    // Exercise the packed JS against this revision's native build, never a registry binary.
    const binding = Object.keys(require.cache).find(file => file.startsWith(ROOT_DIR + path.sep) && file.endsWith('.node'));
    assert.ok(binding, 'Build the current checkout before the packed CLI smoke test.');
    const packedResult = spawnSync(process.execPath, [npm, 'exec', '--offline', '--', 'lazy-image',
      'compile', INPUT, '--out-dir', path.join(parent, 'packed-output'), '--policy', policyPath], {
      cwd: installDir, encoding: 'utf8', timeout: 120_000,
      env: { ...process.env, NAPI_RS_NATIVE_LIBRARY_PATH: binding },
    });
    assert.equal(packedResult.status, 0, packedResult.stderr);
    assert.equal(packedResult.stderr, '');
    assert.deepEqual(JSON.parse(packedResult.stdout), JSON.parse(await fsp.readFile(path.join(parent, 'packed-output/manifest.json'), 'utf8')));
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
