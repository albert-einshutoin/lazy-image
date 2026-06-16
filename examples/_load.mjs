import { createRequire } from 'node:module';
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const require = createRequire(import.meta.url);

export const EXAMPLES_DIR = path.dirname(fileURLToPath(import.meta.url));
export const ROOT_DIR = path.dirname(EXAMPLES_DIR);
export const FIXTURES_DIR = path.join(ROOT_DIR, 'test', 'fixtures');

export function loadLazyImage() {
  let packageError;

  try {
    return require('@alberteinshutoin/lazy-image');
  } catch (err) {
    packageError = err;
  }

  try {
    return require(path.join(ROOT_DIR, 'index.js'));
  } catch (localError) {
    const error = new Error(
      'Unable to load lazy-image. Run `npm run build` before repo-local examples, or install @alberteinshutoin/lazy-image.',
    );
    error.cause = localError;
    error.packageLoadError = packageError;
    throw error;
  }
}

export function hasSelfTestFlag(args = process.argv.slice(2)) {
  return args.includes('--self-test');
}

export function fixturePath(name) {
  return path.join(FIXTURES_DIR, name);
}

export function makeTempDir(prefix) {
  return mkdtempSync(path.join(os.tmpdir(), prefix));
}

export function ensureDir(dir) {
  mkdirSync(dir, { recursive: true });
  return dir;
}

export function removeDir(dir) {
  if (dir && existsSync(dir)) {
    rmSync(dir, { recursive: true, force: true });
  }
}

export function copyFixture(name, outputDir, outputName = name) {
  ensureDir(outputDir);
  const destination = path.join(outputDir, outputName);
  copyFileSync(fixturePath(name), destination);
  return destination;
}
