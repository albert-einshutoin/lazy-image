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
const { writeMetadata } = require('./write-native-metadata.cjs');
const platformByTarget = {
  'x86_64-apple-darwin': 'darwin-x64', 'aarch64-apple-darwin': 'darwin-arm64',
  'x86_64-pc-windows-msvc': 'win32-x64-msvc',
  'x86_64-unknown-linux-gnu': 'linux-x64-gnu', 'aarch64-unknown-linux-gnu': 'linux-arm64-gnu',
  'x86_64-unknown-linux-musl': 'linux-x64-musl',
};
const platform = args.length ? platformByTarget[args[1]] : process.platform === 'linux'
  ? `linux-${process.arch}-${process.report.getReport().header.glibcVersionRuntime ? 'gnu' : 'musl'}`
  : process.platform === 'win32' ? 'win32-x64-msvc' : `${process.platform}-${process.arch}`;
writeMetadata(path.join(root, `lazy-image.${platform}.node`));
