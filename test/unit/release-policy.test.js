const assert = require("node:assert/strict");

const {
  hasBreakingMarker,
  hasRemovedEntry,
  isMajorBoundary,
  parseVersion,
  sectionForVersion,
} = require("../../scripts/check-release-policy");

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
