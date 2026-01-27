const assert = require('assert');
const { ImageEngine } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

async function main() {
  const input = resolveFixture('test_38kb_input.jpg');

  const bufferWithWarnings = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true, exif: true, xmp: true })
    .toBuffer('jpeg', 80);
  assert(bufferWithWarnings.length > 0, 'should produce output when EXIF/XMP requested');

  const bufferIccDisabled = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: false, exif: true })
    .toBuffer('jpeg', 80);
  assert(bufferIccDisabled.length > 0, 'should allow disabling ICC while requesting EXIF');

  console.log('metadata-options.test.js passed');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
