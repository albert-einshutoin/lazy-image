const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');

const root = path.resolve(__dirname, '..');
const pkg = require('../package.json');

function loadFile(file) {
  const metadata = JSON.parse(fs.readFileSync(`${file}.json`, 'utf8'));
  const sha256 = createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  if (metadata.version !== pkg.version || metadata.sha256 !== sha256) {
    throw new Error(`Native binding metadata/version mismatch: ${file}. Rebuild this revision.`);
  }
  return require(file);
}

function platformName() {
  if (process.platform === 'win32' && process.arch === 'x64') return 'win32-x64-msvc';
  if (process.platform === 'darwin' && ['arm64', 'x64'].includes(process.arch)) return `darwin-${process.arch}`;
  if (process.platform === 'linux') {
    process.report.excludeNetwork = true;
    const report = process.report.getReport();
    if (report.header.glibcVersionRuntime) return `linux-${process.arch}-gnu`;
    const musl = report.sharedObjects.some(file => /(?:libc\.musl-|ld-musl-)/.test(file)) ||
      fs.readFileSync('/usr/bin/ldd', 'utf8').includes('musl');
    if (musl) return `linux-${process.arch}-musl`;
    throw new Error('Cannot identify the native Linux libc; no binding selected.');
  }
  throw new Error(`Unsupported native platform: ${process.platform}-${process.arch}`);
}

function loadNative() {
  if (process.env.NAPI_RS_WASI_FLAVOR || ['true', 'error'].includes(process.env.NAPI_RS_FORCE_WASI)) {
    throw new Error('This package ships native bindings only. Use the separate lazy-image-wasm package.');
  }
  const platform = platformName();
  const name = `${pkg.name}-${platform}`;
  if (!Object.hasOwn(pkg.optionalDependencies, name)) throw new Error(`Unsupported native platform: ${platform}`);
  const override = process.env.NAPI_RS_NATIVE_LIBRARY_PATH;
  const local = path.join(root, `lazy-image.${platform}.node`);
  let binding;
  if (override) {
    if (!path.isAbsolute(override) || !override.endsWith('.node')) throw new Error('Native override must be an absolute .node path with build metadata.');
    binding = loadFile(override);
  } else if (fs.existsSync(local)) {
    binding = loadFile(local);
  } else {
    // Check package metadata before evaluating native code. Never cross into a
    // different binary on metadata, initialization or dependency failures.
    let metadata;
    try {
      metadata = require(`${name}/package.json`);
    } catch (cause) {
      throw new Error(`Cannot read native binding metadata: ${name}`, { cause });
    }
    if (metadata.version !== pkg.version) {
      throw new Error(`Native binding package version mismatch: expected ${pkg.version}, got ${metadata.version}`);
    }
    binding = require(name);
  }
  if (typeof binding.version !== 'function' || binding.version() !== pkg.version) {
    throw new Error(`Native runtime version mismatch: expected ${pkg.version}. Reinstall or rebuild this revision.`);
  }
  return binding;
}

module.exports = loadNative();
