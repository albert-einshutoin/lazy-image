import fs from 'node:fs';
import path from 'node:path';
import { loadPublished, here, sha } from './published.mjs';
const [input, out] = process.argv.slice(2);
if (!input || !out) throw Error('Usage: node controls.mjs <input-png> <new-dir>');
const frozen = JSON.parse(fs.readFileSync(path.join(here, 'input.json')));
if (sha(input) !== frozen.sha256) throw Error('Input hash differs from frozen PNG');
const { api, identity } = loadPublished();
fs.mkdirSync(out);
const rows = [];
const base = api.ImageEngine.fromPath(input).sanitize({ policy: 'public-upload' });
for (const width of [684, 960, 1920]) {
  const file = `control-${width}.png`;
  await base.clone().resize({ width, fit: 'inside' }).toFile(path.join(out, file), 'png');
  rows.push({ file, width, bytes: fs.statSync(path.join(out, file)).size, sha256: sha(path.join(out, file)) });
}
fs.writeFileSync(path.join(out, 'controls.json'), JSON.stringify({ scope: 'Lossless low-level PNG controls, outside compiler timing', identity, inputSha256: frozen.sha256, rows }, null, 2) + '\n', { flag: 'wx' });
console.log(JSON.stringify(rows));
