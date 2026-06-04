const fs = require('node:fs');
const { resolveRoot } = require('./paths');

function readPackageJson() {
  return JSON.parse(fs.readFileSync(resolveRoot('package.json'), 'utf8'));
}

function collectBenchmarkPackageImpact({ benchmarkTool, candidateBackend }) {
  const pkg = readPackageJson();
  const optionalDependencyPackages = Object.keys(pkg.optionalDependencies || {}).sort();
  const nativeTargets = Array.isArray(pkg.napi?.targets) ? [...pkg.napi.targets].sort() : [];

  return {
    packageName: pkg.name,
    packageVersion: pkg.version,
    npmFiles: Array.isArray(pkg.files) ? [...pkg.files] : [],
    optionalDependencyPackages,
    optionalDependencyCount: optionalDependencyPackages.length,
    nativeTargets,
    nativeTargetCount: nativeTargets.length,
    publicApiChanged: false,
    runtimePackageChanged: false,
    productionBuildDependencyChanged: false,
    benchmarkTool,
    candidateBackend,
    benchmarkToolingScope:
      `${benchmarkTool} is an operator-provided benchmark CLI and is not linked into lazy-image native packages.`,
    crossPlatformImpact: {
      macos: 'unchanged by this benchmark harness',
      linux: 'unchanged by this benchmark harness',
      windows: 'unchanged by this benchmark harness',
    },
    futureProductionSwitchRequires: [
      `Measure macOS, Linux, and Windows native package size after linking ${candidateBackend}.`,
      `Measure CI build time after adding ${candidateBackend} as a production build dependency.`,
      'Keep npm package files and optional platform packages stable unless the backend switch PR explicitly changes them.',
    ],
  };
}

function buildPackageImpactMarkdown(packageImpact) {
  const targets = packageImpact.nativeTargets.length ? packageImpact.nativeTargets.join(', ') : 'none';
  const npmFiles = packageImpact.npmFiles.length ? packageImpact.npmFiles.join(', ') : 'none';

  return [
    '## Package / Build Impact',
    '',
    `Package: ${packageImpact.packageName}@${packageImpact.packageVersion}`,
    `Benchmark tool: ${packageImpact.benchmarkTool}`,
    `Candidate backend: ${packageImpact.candidateBackend}`,
    `Runtime package changed: ${packageImpact.runtimePackageChanged ? 'yes' : 'no'}`,
    `Production build dependency changed: ${packageImpact.productionBuildDependencyChanged ? 'yes' : 'no'}`,
    `Public API changed: ${packageImpact.publicApiChanged ? 'yes' : 'no'}`,
    `NAPI targets (${packageImpact.nativeTargetCount}): ${targets}`,
    `Optional platform packages: ${packageImpact.optionalDependencyCount}`,
    `npm files allowlist: ${npmFiles}`,
    `macOS impact: ${packageImpact.crossPlatformImpact.macos}`,
    `Linux impact: ${packageImpact.crossPlatformImpact.linux}`,
    `Windows impact: ${packageImpact.crossPlatformImpact.windows}`,
    `Tooling scope: ${packageImpact.benchmarkToolingScope}`,
    '',
    'Future production switch evidence required:',
    ...packageImpact.futureProductionSwitchRequires.map((item) => `- ${item}`),
  ];
}

module.exports = {
  buildPackageImpactMarkdown,
  collectBenchmarkPackageImpact,
};
