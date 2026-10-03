const { spawnSync } = require('node:child_process');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const targets = require('../package.json').napi.targets;
const args = process.argv.slice(2);
if (args.length !== 0 && (args.length !== 2 || args[0] !== '--target' || !targets.includes(args[1]))) {
  throw new Error(`Expected --target <supported Rust target>; received ${args.join(' ')}`);
}

function run(script, scriptArgs) {
  const result = spawnSync(process.execPath, [script, ...scriptArgs], { cwd: root, stdio: 'inherit', env: process.env });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status || 1);
}

run(path.join(root, 'scripts/postbuild-fixup.js'), ['--prepare']);
run(path.join(root, 'node_modules/@napi-rs/cli/dist/cli.js'), ['build', '--platform', '--release', ...args]);
run(path.join(root, 'scripts/postbuild-fixup.js'), []);
