#!/usr/bin/env node
'use strict';

const fsp = require('node:fs/promises');
const { parseArgs } = require('node:util');

const USAGE = 'Usage: lazy-image compile <input> --out-dir <new-directory> --policy <policy.json>';

class CliUsageError extends Error {
  constructor(message) {
    super(message);
    this.name = 'CliUsageError';
  }
}

function parseCliArgs(argv) {
  let parsed;
  try {
    parsed = parseArgs({
      args: argv,
      options: {
        'out-dir': { type: 'string', multiple: true },
        policy: { type: 'string', multiple: true },
      },
      allowPositionals: true,
      strict: true,
    });
  } catch (error) {
    throw new CliUsageError(error.message);
  }

  const { positionals, values } = parsed;
  if (positionals.length !== 2 || positionals[0] !== 'compile' || positionals[1].length === 0) {
    throw new CliUsageError('Expected the compile command and exactly one input path.');
  }

  const readSingleOption = (name) => {
    const entries = values[name];
    if (!Array.isArray(entries) || entries.length !== 1 || entries[0].length === 0) {
      throw new CliUsageError(`${name} must be provided exactly once.`);
    }
    return entries[0];
  };

  return {
    inputPath: positionals[1],
    outputDir: readSingleOption('out-dir'),
    policyPath: readSingleOption('policy'),
  };
}

async function readPolicy(policyPath) {
  let source;
  try {
    source = await fsp.readFile(policyPath, 'utf8');
  } catch (error) {
    throw new CliUsageError(`Could not read policy file: ${error.message}`);
  }

  try {
    return JSON.parse(source);
  } catch (error) {
    throw new CliUsageError(`Policy JSON is invalid: ${error.message}`);
  }
}

function formatDiagnostic(error) {
  const errorCode = error && (error.errorCode || error.code);
  const prefix = errorCode ? `[${errorCode}] ` : '';
  const phase = error && error.phase ? ` (${error.phase})` : '';
  const message = error && error.message ? error.message : String(error);
  const lines = [`${prefix}${message}${phase}`];
  if (error && error.recoveryHint) lines.push(`Hint: ${error.recoveryHint}`);
  if (error instanceof CliUsageError) lines.push(USAGE);
  return lines.join('\n');
}

async function main(argv = process.argv.slice(2)) {
  let cli;
  let policy;
  try {
    cli = parseCliArgs(argv);
    policy = await readPolicy(cli.policyPath);
  } catch (error) {
    process.stderr.write(`${formatDiagnostic(error)}\n`);
    return 2;
  }

  const controller = new AbortController();
  const abort = () => controller.abort();
  process.on('SIGINT', abort);
  process.on('SIGTERM', abort);
  try {
    // Load the native package only after CLI and policy syntax have passed.
    const { compileImage } = require('../index');
    const manifest = await compileImage({
      inputPath: cli.inputPath,
      outputDir: cli.outputDir,
      policy,
      signal: controller.signal,
    });
    process.stdout.write(`${JSON.stringify(manifest)}\n`);
    return 0;
  } catch (error) {
    process.stderr.write(`${formatDiagnostic(error)}\n`);
    return 1;
  } finally {
    process.removeListener('SIGINT', abort);
    process.removeListener('SIGTERM', abort);
  }
}

if (require.main === module) {
  main().then(
    (exitCode) => { process.exitCode = exitCode; },
    (error) => {
      process.stderr.write(`${formatDiagnostic(error)}\n`);
      process.exitCode = 1;
    },
  );
}

module.exports = { formatDiagnostic, main, parseCliArgs, readPolicy };
