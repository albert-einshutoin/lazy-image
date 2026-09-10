const assert = require('node:assert/strict');
const { readCorpusManifest, verifyCorpusManifest } = require('../helpers/benchmark-corpus');
const { resolveRoot } = require('../helpers/paths');
const file = resolveRoot('test/benchmarks/corpus/release-manifest.json');
const manifest = readCorpusManifest(file);
const corpus = verifyCorpusManifest(manifest);
for (const category of ['photo', 'ui', 'illustration', 'alpha', 'metadata', 'hostile']) {
  assert.ok(corpus.entries.filter(e => e.category === category).length >= 2, category);
}
const changed = structuredClone(manifest);
changed.additionalLicenses[0].sha256 = '0'.repeat(64);
assert.throws(() => verifyCorpusManifest(changed), /license checksum/);
console.log('release corpus coverage and license checks passed');
const { validateEvidence } = require('../benchmarks/release-evidence');
assert.throws(()=>validateEvidence({corpus:{entries:[]}},{}), /category coverage/);
