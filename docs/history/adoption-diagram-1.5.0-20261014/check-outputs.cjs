const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const [repository, phase] = process.argv.slice(2);
if (!repository || !phase) throw Error('Usage: node check-outputs.cjs <repository> <phase-dir>');
const sharp = require(require.resolve('sharp', { paths: [path.resolve(repository)] }));
const read = file => JSON.parse(fs.readFileSync(file));
const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
(async () => {
  const consumer = path.join(phase, '..', 'consumer');
  const identity = read(path.join(consumer, 'identity.json')), input = read(path.join(consumer, 'input.json'));
  const codeHashes = ['published.mjs', 'controls.mjs', 'compile.mjs', 'measure.mjs'].map(file => ({ file, sha256: hash(path.join(consumer, file)) }));
  assert.equal(hash(path.join(consumer, 'package-lock.json')), identity.lockSha256);
  assert.equal(hash(path.join(phase, 'raster/diagram-input.png')), input.sha256);
  const saved = process.argv[4] === '--check';
  const commandRoot = saved ? '<PRIVATE_CONSUMER>' : path.resolve(consumer);
  const phaseRoot = saved ? '<PRIVATE_PHASE>' : path.resolve(phase);
  const node = saved ? '<NODE_EXECUTABLE>' : process.execPath;
  const processes = read(path.join(phase, 'runs/processes.json'));
  assert.deepEqual(processes.map(row => row.name), ['A1', 'B1', 'B2', 'B3']);
  const summaries = [];
  for (const process of processes) {
    const report = read(path.join(phase, `runs/${process.name}.json`));
    assert.equal(process.evaluationRevision, 'a8aa32716c8ce9fa4078808a9424e12969171a7f');
    assert.deepEqual(process.codeHashes, codeHashes);
    assert.deepEqual(process.command, [node, `${commandRoot}/compile.mjs`, process.name[0], `${phaseRoot}/raster/diagram-input.png`, `${phaseRoot}/runs/${process.name}`, `${phaseRoot}/runs/${process.name}.json`]);
    assert.equal(process.spawnError, null);
    assert.deepEqual(read(path.join(phase, `runs/${process.name}.process.json`)), process);
    assert.equal(fs.readFileSync(path.join(phase, `runs/${process.name}.exit`), 'utf8').trim(), '0');
    assert.equal(report.inputSha256, input.sha256);
    const policyFile = path.join(consumer, `policy-${process.name[0]}.json`);
    assert.equal(report.policySha256, hash(policyFile)); assert.deepEqual(report.policy, read(policyFile));
    assert.equal(report.identity.sourceSha, identity.sourceSha); assert.equal(report.identity.version, '1.5.0');
    assert.equal(report.identity.nativeSha256, identity.nativeSha256); assert.equal(report.identity.lockSha256, identity.lockSha256);
    assert.deepEqual(report.identity.loadedJs, identity.loadedJs); assert.deepEqual(report.identity.overrides, []);
    const actualFiles = fs.readdirSync(path.join(phase, 'runs', process.name)).map(file => ({ file,
      bytes: fs.statSync(path.join(phase, 'runs', process.name, file)).size, sha256: hash(path.join(phase, 'runs', process.name, file)) }));
    assert.deepEqual(process.files, actualFiles);
    assert.equal(process.exitCode, 0); assert.equal(process.signal, null);
    assert.equal(report.success, true); assert.deepEqual(report.remainingStaging, []);
    assert.equal(report.manifest.compiler.version, '1.5.0');
    assert.deepEqual(report.manifest, read(path.join(phase, 'runs', process.name, 'manifest.json')));
    assert.equal(report.imageStageMs, process.imageStageMs);
    assert.ok(Number.isFinite(process.imageStageMs) && process.imageStageMs >= 0);
    if (process.name.startsWith('B')) assert.ok(process.imageStageMs <= 60000);
    assert.equal(report.manifest.artifacts.length, 6);
    assert.deepEqual(report.manifest.artifacts.map(a => a.id).sort(), ['1920/jpeg', '1920/webp', '684/jpeg', '684/webp', '960/jpeg', '960/webp'].sort());
    const artifacts = [];
    for (const artifact of report.manifest.artifacts) {
      assert.equal(path.basename(artifact.path), artifact.path);
      const file = path.join(phase, 'runs', process.name, artifact.path);
      assert.equal(fs.statSync(file).size, artifact.bytes); assert.equal(hash(file), artifact.sha256);
      const metadata = await sharp(file).metadata();
      assert.equal(metadata.width, artifact.width); assert.equal(metadata.height, artifact.height); assert.equal(metadata.format, artifact.format);
      assert.equal(artifact.quality, artifact.format === 'webp' ? 80 : 85);
      if (process.name.startsWith('B')) {
        const budget = report.policy.budgets.find(b => b.width === artifact.width && b.format === artifact.format);
        assert.equal(artifact.targetBytes, budget.maxBytes); assert.ok(artifact.bytes <= budget.maxBytes);
        assert.ok(artifact.quality >= 30 && artifact.quality <= budget.maxQuality);
      }
      artifacts.push({ ...artifact, encodedMetadata: metadata });
    }
    summaries.push({ name: process.name, exitCode: process.exitCode, imageStageMs: process.imageStageMs, processWallMs: process.processWallMs, artifacts });
  }
  for (const width of [684, 960, 1920]) {
    const metadata = await sharp(path.join(phase, 'controls', `control-${width}.png`)).metadata();
    const expected = summaries[1].artifacts.find(a => a.width === width);
    assert.equal(metadata.format, 'png'); assert.equal(metadata.width, width); assert.equal(metadata.height, expected.height);
  }
  const imagesMatch = summaries.every(row => row.artifacts.every(a => a.sha256 === summaries[0].artifacts.find(b => b.id === a.id).sha256));
  const result = { scope: 'Recorded four successful calls; failures remain in raw if a replay differs',
    normalCalls: 4, normalArtifacts: 24, budgetArtifacts: 18, budgetFits: 18, E123: 0, E300: 0,
    allImageHashesMatch: imagesMatch, summaries };
  if (process.argv[4] === '--check') assert.deepEqual(read(path.join(phase, 'output-check.json')), result);
  else fs.writeFileSync(path.join(phase, 'output-check.json'), JSON.stringify(result, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ normalCalls: 4, normalArtifacts: 24, budgetFits: 18, allImageHashesMatch: imagesMatch }));
})().catch(error => { console.error(error); process.exitCode = 1; });
