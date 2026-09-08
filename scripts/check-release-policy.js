const fs = require("fs");
const path = require("path");

const root = path.resolve(__dirname, "..");
const pkgPath = path.join(root, "package.json");
const changelogPath = path.join(root, "CHANGELOG.md");

function readJson(filePath) {
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`Failed to parse JSON file ${filePath}: ${message}`);
  }
}

function parseVersion(version) {
  const match = /^(\d+)\.(\d+)\.(\d+)(?:[-+].*)?$/.exec(version);
  if (!match) {
    throw new Error(`Invalid semver version: ${version}`);
  }
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
}

function sectionForVersion(changelog, version) {
  const releaseHeading = new RegExp(`^## \\[${escapeRegExp(version)}\\].*$`, "m");
  const releaseMatch = releaseHeading.exec(changelog);
  if (releaseMatch) {
    return sliceSection(changelog, releaseMatch.index + releaseMatch[0].length);
  }

  const unreleasedMatch = /^## \[Unreleased\].*$/m.exec(changelog);
  if (!unreleasedMatch) {
    throw new Error("CHANGELOG.md is missing an [Unreleased] section");
  }
  return sliceSection(changelog, unreleasedMatch.index + unreleasedMatch[0].length);
}

function sliceSection(changelog, start) {
  const nextHeading = /^## \[/m.exec(changelog.slice(start));
  if (!nextHeading) {
    return changelog.slice(start);
  }
  return changelog.slice(start, start + nextHeading.index);
}

function hasBreakingMarker(section) {
  return (
    /^### .*\bBREAKING\b.*$/m.test(section) ||
    /^\s*-\s+.*\bBREAKING\b.*$/m.test(section) ||
    hasRemovedEntry(section)
  );
}

function hasRemovedEntry(section) {
  const removedHeading = /^### Removed\s*$/m.exec(section);
  if (!removedHeading) {
    return false;
  }

  const removedBody = sliceSubsection(
    section,
    removedHeading.index + removedHeading[0].length
  );
  return /^\s*-\s+\S/m.test(removedBody);
}

function sliceSubsection(section, start) {
  const nextHeading = /^### /m.exec(section.slice(start));
  if (!nextHeading) {
    return section.slice(start);
  }
  return section.slice(start, start + nextHeading.index);
}

function isMajorBoundary(version) {
  return version.minor === 0 && version.patch === 0;
}

function assertTagMatchesVersion(tag, version) {
  if (tag !== `v${version}`) {
    throw new Error(`Release tag ${tag} does not match package version ${version}.`);
  }
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

function main() {
  const pkg = readJson(pkgPath);
  const version = process.argv[2] || pkg.version;
  const tag = process.argv[3];
  if (tag) {
    assertTagMatchesVersion(tag, version);
  }
  const parsed = parseVersion(version);
  const changelog = fs.readFileSync(changelogPath, "utf8");
  const section = sectionForVersion(changelog, version);

  if (!hasBreakingMarker(section)) {
    console.log(`Release policy check passed for ${version}: no breaking marker found.`);
    return;
  }

  if (isMajorBoundary(parsed)) {
    console.log(`Release policy check passed for ${version}: breaking changes use a major boundary.`);
    return;
  }

  throw new Error(
    [
      `Release ${version} contains a breaking-change marker but is not a major boundary.`,
      "Move the release to x.0.0 (for current 0.x development, that means 1.0.0)",
      "or reclassify the CHANGELOG entry as a bug-fix clarification before release.",
    ].join(" ")
  );
}

if (require.main === module) {
  try {
    main();
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    console.error(`Release policy check failed: ${message}`);
    process.exit(1);
  }
}

module.exports = {
  assertTagMatchesVersion,
  hasBreakingMarker,
  hasRemovedEntry,
  isMajorBoundary,
  parseVersion,
  sectionForVersion,
};
