const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const {
  assertTagMatchesVersion,
  hasBreakingMarker,
  hasRemovedEntry,
  isMajorBoundary,
  parseVersion,
  sectionForVersion,
} = require("../../scripts/check-release-policy");

const workflow = fs.readFileSync(
  path.join(__dirname, "../../.github/workflows/CI.yml"),
  "utf8",
);

function test(name, fn) {
  try {
    fn();
    console.log(`ok - ${name}`);
  } catch (error) {
    console.error(`not ok - ${name}`);
    throw error;
  }
}

test("BREAKING markers are case-sensitive release markers", () => {
  assert.equal(hasBreakingMarker("### Changed\n- non-breaking docs update"), false);
  assert.equal(hasBreakingMarker("### Changed\n- breaking clarification only"), false);
  assert.equal(hasBreakingMarker("### Changed (BREAKING)\n- PNG quality is rejected"), true);
  assert.equal(hasBreakingMarker("### Changed\n- BREAKING: PNG quality is rejected"), true);
});

test("Removed section only blocks when it has entries", () => {
  assert.equal(hasRemovedEntry("### Removed\n\n### Fixed\n- bug"), false);
  assert.equal(hasRemovedEntry("### Removed\n- Deprecated API"), true);
});

test("target version section is preferred over Unreleased", () => {
  const changelog = [
    "## [Unreleased]",
    "### Changed (BREAKING)",
    "- Future breaking change",
    "",
    "## [0.16.0] - 2026-05-29",
    "### Fixed",
    "- non-breaking patch note",
    "",
    "## [0.15.0] - 2026-05-28",
  ].join("\n");

  assert.match(sectionForVersion(changelog, "0.16.0"), /non-breaking patch note/);
  assert.doesNotMatch(sectionForVersion(changelog, "0.16.0"), /Future breaking change/);
});

test("major boundary means x.0.0", () => {
  assert.equal(isMajorBoundary(parseVersion("1.0.0")), true);
  assert.equal(isMajorBoundary(parseVersion("2.0.0-beta.1")), true);
  assert.equal(isMajorBoundary(parseVersion("0.16.0")), false);
  assert.equal(isMajorBoundary(parseVersion("1.2.0")), false);
});

test("release tag must match the package version", () => {
  assert.doesNotThrow(() => assertTagMatchesVersion("v1.0.0", "1.0.0"));
  assert.throws(
    () => assertTagMatchesVersion("v1.0.1", "1.0.0"),
    /Release tag v1\.0\.1 does not match package version 1\.0\.0/,
  );
});

test("manual release workflow can only enter the publish job in dry-run mode", () => {
  assert.match(
    workflow,
    /if: startsWith\(github\.ref, 'refs\/tags\/v'\) \|\| \(github\.event_name == 'workflow_dispatch' && !inputs\.full_validation && inputs\.dry_run\)/,
  );
});

test("main package publish skips an existing version", () => {
  const mainPublish = workflow.slice(workflow.indexOf("- name: Publish main package"));
  assert.match(mainPublish, /npm view "\$pkg_name@\$pkg_version" version/);
  assert.ok(mainPublish.indexOf("npm view") < mainPublish.indexOf("npm publish"));
});
