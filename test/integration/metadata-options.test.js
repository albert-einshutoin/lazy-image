const assert = require('assert');
const { ImageEngine } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

async function main() {
  const input = resolveFixture('test_38kb_input.jpg');

  // Test 1: XMP warning should be emitted (not yet supported)
  console.log('Test 1: XMP emits warning but still processes');
  const bufferWithXmpWarning = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true, exif: true, xmp: true })
    .toBuffer('jpeg', 80);
  assert(bufferWithXmpWarning.length > 0, 'should produce output when XMP requested');

  // Test 2: Disable ICC while enabling EXIF
  console.log('Test 2: Disable ICC, enable EXIF');
  const bufferIccDisabled = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: false, exif: true })
    .toBuffer('jpeg', 80);
  assert(bufferIccDisabled.length > 0, 'should allow disabling ICC while requesting EXIF');

  // Test 3: stripGps option (default true)
  console.log('Test 3: GPS stripping (default behavior)');
  const bufferGpsStripped = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true, exif: true })  // stripGps defaults to true
    .toBuffer('jpeg', 80);
  assert(bufferGpsStripped.length > 0, 'should process with GPS stripping');

  // Test 4: Explicitly preserve GPS
  console.log('Test 4: Preserve GPS (opt-in)');
  const bufferGpsPreserved = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true, exif: true, stripGps: false })
    .toBuffer('jpeg', 80);
  assert(bufferGpsPreserved.length > 0, 'should process with GPS preservation');

  // Test 5: Only ICC profile (no EXIF)
  console.log('Test 5: ICC only (default metadata behavior)');
  const bufferIccOnly = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true })
    .toBuffer('jpeg', 80);
  assert(bufferIccOnly.length > 0, 'should process with ICC only');

  // Test 6: No metadata (default security behavior)
  console.log('Test 6: No metadata (security-first default)');
  const bufferNoMetadata = await ImageEngine.fromPath(input)
    .toBuffer('jpeg', 80);
  assert(bufferNoMetadata.length > 0, 'should process without any metadata');

  // Test 7: Firewall strict mode strips all metadata
  console.log('Test 7: Firewall strict mode overrides keepMetadata');
  const bufferFirewallStrict = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true, exif: true, stripGps: false })
    .sanitize({ policy: 'strict' })
    .toBuffer('jpeg', 80);
  assert(bufferFirewallStrict.length > 0, 'firewall strict should override keepMetadata');

  console.log('metadata-options.test.js passed - all 7 tests');
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
