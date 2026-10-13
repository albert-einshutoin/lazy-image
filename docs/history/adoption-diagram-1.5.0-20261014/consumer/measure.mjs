// Fresh-process wall/exit capture follows the #902 runner; this workload is one new diagram.
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { performance } from 'node:perf_hooks';
import { here, sha } from './published.mjs';
const [input, out] = process.argv.slice(2);
if (!input || !out || !process.env.EVALUATION_REVISION) throw Error('Usage: EVALUATION_REVISION=<sha> node measure.mjs <input-png> <new-runs>');
fs.mkdirSync(out);
const codeHashes = ['published.mjs', 'controls.mjs', 'compile.mjs', 'measure.mjs'].map(file => ({ file, sha256: sha(path.join(here, file)) }));
const rows = [];
for (const name of ['A1', 'B1', 'B2', 'B3']) {
  const site = path.join(out, name), reportFile = path.join(out, `${name}.json`);
  const args = [path.join(here, 'compile.mjs'), name[0], path.resolve(input), path.resolve(site), path.resolve(reportFile)];
  const started = performance.now();
  const result = spawnSync(process.execPath, args, { cwd: here, encoding: 'utf8' });
  const processWallMs = performance.now() - started;
  fs.writeFileSync(path.join(out, `${name}.stdout.log`), result.stdout ?? '', { flag: 'wx' });
  fs.writeFileSync(path.join(out, `${name}.stderr.log`), result.stderr ?? '', { flag: 'wx' });
  fs.writeFileSync(path.join(out, `${name}.exit`), `${result.status}\n`, { flag: 'wx' });
  const observed = fs.existsSync(reportFile) ? JSON.parse(fs.readFileSync(reportFile)) : null;
  const files = fs.existsSync(site) ? fs.readdirSync(site).map(file => ({ file, bytes: fs.statSync(path.join(site, file)).size, sha256: sha(path.join(site, file)) })) : [];
  const row = { name, command: [process.execPath, ...args], evaluationRevision: process.env.EVALUATION_REVISION,
    codeHashes, processWallMs, exitCode: result.status, signal: result.signal,
    spawnError: result.error?.message ?? null, imageStageMs: observed?.imageStageMs ?? null,
    success: observed?.success ?? false, files };
  rows.push(row);
  fs.writeFileSync(path.join(out, `${name}.process.json`), JSON.stringify(row, null, 2) + '\n', { flag: 'wx' });
  console.log(JSON.stringify({ name, exit: row.exitCode, processWallMs, imageStageMs: row.imageStageMs }));
}
fs.writeFileSync(path.join(out, 'processes.json'), JSON.stringify(rows, null, 2) + '\n', { flag: 'wx' });
process.exitCode = rows.every(row => row.exitCode === 0) ? 0 : 1;
