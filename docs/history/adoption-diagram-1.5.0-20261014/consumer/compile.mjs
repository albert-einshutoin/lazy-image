import fs from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { loadPublished, here, sha } from './published.mjs';
const [condition, input, out, reportFile] = process.argv.slice(2);
if (!['A', 'B'].includes(condition) || !input || !out || !reportFile) throw Error('Usage: node compile.mjs A|B <input-png> <new-output> <new-report>');
const policyFile = path.join(here, `policy-${condition}.json`);
const policy = JSON.parse(fs.readFileSync(policyFile));
const frozen = JSON.parse(fs.readFileSync(path.join(here, 'input.json')));
const report = { condition, inputSha256: sha(input), policy, policySha256: sha(policyFile), success: false,
  imageStageMs: null, nativeMetrics: null, nativeMetricsReason: 'compileImage manifest does not expose native metrics' };
const errorFacts = error => ({ name: error.name, message: error.message, phase: error.phase ?? null,
  errorCode: error.errorCode ?? null, code: error.code ?? null,
  cause: error.cause ? errorFacts(error.cause) : null, cleanupError: error.cleanupError ? errorFacts(error.cleanupError) : null });
try {
  if (report.inputSha256 !== frozen.sha256) throw Error('Input hash differs from frozen PNG');
  const { api, identity } = loadPublished();
  report.identity = identity;
  const started = performance.now();
  try {
    report.manifest = await api.compileImage({ inputPath: path.resolve(input), outputDir: path.resolve(out), policy });
    report.success = true;
  } finally { report.imageStageMs = performance.now() - started; }
} catch (error) { report.error = errorFacts(error); }
report.outputExists = fs.existsSync(out);
report.remainingStaging = fs.readdirSync(path.dirname(path.resolve(out))).filter(name => name.startsWith(`.${path.basename(out)}.lazy-image-staging-`));
fs.writeFileSync(reportFile, JSON.stringify(report, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify({ condition, success: report.success, imageStageMs: report.imageStageMs, error: report.error ?? null }));
process.exitCode = report.success ? 0 : 1;
