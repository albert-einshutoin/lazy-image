'use strict';

const { adapterForManifest } = require('./adapters');
const { matchesAny } = require('./patterns');

function uniqueSorted(values) {
  return [...new Set(values)].sort();
}

function detectProjects(files) {
  const projects = new Map();
  for (const file of files) {
    const adapter = adapterForManifest(file);
    if (!adapter) continue;
    const slash = file.lastIndexOf('/');
    const root = slash === -1 ? '.' : file.slice(0, slash);
    const key = `${adapter.id}:${root}`;
    if (!projects.has(key)) {
      projects.set(key, { name: `${root}:${adapter.id}`, root, adapter: adapter.id, manifest: file });
    }
  }
  return [...projects.values()];
}

function fullPlan({ baseRevision, headRevision, changes, projects, reason }) {
  return {
    strategy: 'full',
    baseRevision,
    headRevision,
    changedFiles: changes.map(change => change.path),
    detectedProjects: projects,
    affectedProjects: projects.map(project => project.name),
    affectedModules: [],
    unitTestTargets: [],
    integrationTestTargets: [],
    e2eTestTargets: [],
    smokeTestTargets: [],
    fallback: true,
    fallbackReason: reason,
  };
}

function affectedModules(changes, modules) {
  const directlyChanged = new Set();
  for (const change of changes) {
    for (const module of modules) {
      if (matchesAny(change.path, module.paths || [])) directlyChanged.add(module.name);
    }
  }

  // Edges describe "module depends on dependency". Walking the reverse graph
  // is intentional: a low-level change must select every upstream consumer.
  const affected = new Set(directlyChanged);
  let expanded = true;
  while (expanded) {
    expanded = false;
    for (const module of modules) {
      if (affected.has(module.name)) continue;
      if ((module.dependsOn || []).some(dependency => affected.has(dependency))) {
        affected.add(module.name);
        expanded = true;
      }
    }
  }
  return uniqueSorted(affected);
}

function planSelectiveTests({ config, changes, baseRevision, headRevision, projects = [] }) {
  if (!Array.isArray(changes) || changes.length === 0) {
    return fullPlan({ baseRevision, headRevision, changes: changes || [], projects, reason: 'no changes could be classified' });
  }

  for (const rule of config.fullTestRules || []) {
    const matched = changes.find(change => matchesAny(change.path, [rule.pattern]));
    if (matched) {
      return fullPlan({
        baseRevision,
        headRevision,
        changes,
        projects,
        reason: `${rule.reason}: ${matched.path}`,
      });
    }
  }

  const modules = affectedModules(changes, config.modules || []);
  const changedTests = changes
    .filter(change => change.exists && /(^|\/)test(s)?\//.test(change.path) && /\.(test|spec)\.[^.]+$/.test(change.path))
    .map(change => change.path);
  const changedUnitTests = changedTests.filter(file => file.includes('/unit/'));
  const changedIntegrationTests = changedTests.filter(file => file.includes('/integration/'));
  const sourceChanges = changes.filter(change =>
    (config.knownSourceRoots || []).some(root => change.path.startsWith(root)),
  );
  const classifiedSourceFiles = sourceChanges.filter(change =>
    (config.modules || []).some(module => matchesAny(change.path, module.paths || [])),
  );
  if (classifiedSourceFiles.length !== sourceChanges.length) {
    const unknown = sourceChanges.find(change => !classifiedSourceFiles.includes(change));
    return fullPlan({
      baseRevision,
      headRevision,
      changes,
      projects,
      reason: `unclassified source file changed: ${unknown.path}`,
    });
  }

  const unmatchedChange = changes.find(change => {
    const isMapped = (config.modules || []).some(module => matchesAny(change.path, module.paths || []));
    const isTest = /(^|\/)test(s)?\//.test(change.path) && /\.(test|spec)\.[^.]+$/.test(change.path);
    return !isMapped && !isTest;
  });
  if (unmatchedChange) {
    return fullPlan({
      baseRevision,
      headRevision,
      changes,
      projects,
      reason: `unclassified file changed: ${unmatchedChange.path}`,
    });
  }

  const selectedModules = (config.modules || []).filter(module => modules.includes(module.name));
  const unitTestTargets = uniqueSorted([
    ...selectedModules.flatMap(module => module.unitTests || []),
    ...changedUnitTests.map(file => `javascript::${file}`),
  ]);
  const integrationTestTargets = uniqueSorted([
    ...selectedModules.flatMap(module => module.integrationTests || []),
    ...changedIntegrationTests,
  ]);
  const e2eTestTargets = uniqueSorted(selectedModules.flatMap(module => module.e2eTests || []));
  const smokeTestTargets = uniqueSorted(config.smokeTests || []);
  const targetCount = unitTestTargets.length + integrationTestTargets.length
    + e2eTestTargets.length + smokeTestTargets.length;

  if (targetCount === 0) {
    return fullPlan({
      baseRevision,
      headRevision,
      changes,
      projects,
      reason: 'changes exist but no safe test targets were selected',
    });
  }

  const affectedProjects = uniqueSorted(projects
    .filter(project => changes.some(change => project.root === '.' || change.path.startsWith(`${project.root}/`)))
    .map(project => project.name));

  return {
    strategy: 'selective',
    baseRevision,
    headRevision,
    changedFiles: changes.map(change => change.path),
    detectedProjects: projects,
    affectedProjects,
    affectedModules: modules,
    unitTestTargets,
    integrationTestTargets,
    e2eTestTargets,
    smokeTestTargets,
    fallback: false,
    fallbackReason: null,
  };
}

module.exports = { detectProjects, planSelectiveTests };
