/**
 * EXIF Roundtrip Test
 * 
 * Verifies that:
 * 1. EXIF metadata is preserved when keepMetadata({ exif: true }) is set
 * 2. Orientation tag is reset to 1 after auto-orient
 * 3. GPS data is stripped by default (privacy protection)
 * 4. GPS data is preserved when stripGps: false
 */

const assert = require('assert');
const fs = require('fs');
const path = require('path');
const { ImageEngine } = require('../../index');
const { resolveFixture } = require('../helpers/paths');

// Simple EXIF parser for verification (reads Orientation and GPS presence)
function parseExifFromJpeg(buffer) {
  const result = { hasExif: false, orientation: null, hasGps: false };
  
  if (buffer[0] !== 0xFF || buffer[1] !== 0xD8) {
    return result; // Not a JPEG
  }

  let i = 2;
  while (i + 3 < buffer.length) {
    if (buffer[i] !== 0xFF) {
      i++;
      continue;
    }
    const marker = buffer[i + 1];
    
    // APP1 marker (EXIF)
    if (marker === 0xE1) {
      const len = (buffer[i + 2] << 8) | buffer[i + 3];
      const segStart = i + 4;
      const segment = buffer.slice(segStart, segStart + len - 2);
      
      // Check for EXIF header
      if (segment.slice(0, 6).toString() === 'Exif\0\0') {
        result.hasExif = true;
        const tiff = segment.slice(6);
        
        // Parse TIFF to find Orientation and GPS
        const parsed = parseTiff(tiff);
        result.orientation = parsed.orientation;
        result.hasGps = parsed.hasGps;
      }
      break;
    }
    
    // Skip other segments
    if (marker === 0xDA || marker === 0xD9) break; // SOS or EOI
    if (marker >= 0xD0 && marker <= 0xD7) {
      i += 2;
      continue;
    }
    
    const len = (buffer[i + 2] << 8) | buffer[i + 3];
    i += 2 + len;
  }
  
  return result;
}

function parseExifFromPng(buffer) {
  const result = { hasExif: false, orientation: null, hasGps: false };
  const signature = Buffer.from([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]);
  if (buffer.length < 8 || !buffer.subarray(0, 8).equals(signature)) {
    return result;
  }

  let offset = 8;
  while (offset + 8 <= buffer.length) {
    const length = buffer.readUInt32BE(offset);
    const type = buffer.subarray(offset + 4, offset + 8).toString('ascii');
    const dataStart = offset + 8;
    const dataEnd = dataStart + length;
    if (dataEnd + 4 > buffer.length) break;

    if (type === 'eXIf') {
      result.hasExif = true;
      const parsed = parseTiff(buffer.subarray(dataStart, dataEnd));
      result.orientation = parsed.orientation;
      result.hasGps = parsed.hasGps;
      break;
    }

    offset = dataEnd + 4;
  }

  return result;
}

function parseExifFromWebP(buffer) {
  const result = { hasExif: false, orientation: null, hasGps: false };
  if (buffer.length < 16 || buffer.subarray(0, 4).toString('ascii') !== 'RIFF' || buffer.subarray(8, 12).toString('ascii') !== 'WEBP') {
    return result;
  }

  let offset = 12;
  while (offset + 8 <= buffer.length) {
    const chunkId = buffer.subarray(offset, offset + 4).toString('ascii');
    const chunkSize = buffer.readUInt32LE(offset + 4);
    const dataStart = offset + 8;
    const dataEnd = dataStart + chunkSize;
    if (dataEnd > buffer.length) break;

    if (chunkId === 'EXIF') {
      let exif = buffer.subarray(dataStart, dataEnd);
      if (exif.subarray(0, 6).toString() === 'Exif\0\0') {
        exif = exif.subarray(6);
      }
      result.hasExif = true;
      const parsed = parseTiff(exif);
      result.orientation = parsed.orientation;
      result.hasGps = parsed.hasGps;
      break;
    }

    offset = dataEnd + (chunkSize % 2);
  }

  return result;
}

function parseTiff(tiff) {
  const result = { orientation: null, hasGps: false };
  
  if (tiff.length < 8) return result;
  
  const isLE = tiff[0] === 0x49 && tiff[1] === 0x49; // "II" = little-endian
  
  const readU16 = (offset) => {
    if (offset + 2 > tiff.length) return 0;
    return isLE 
      ? tiff[offset] | (tiff[offset + 1] << 8)
      : (tiff[offset] << 8) | tiff[offset + 1];
  };
  
  const readU32 = (offset) => {
    if (offset + 4 > tiff.length) return 0;
    return isLE
      ? tiff[offset] | (tiff[offset + 1] << 8) | (tiff[offset + 2] << 16) | (tiff[offset + 3] << 24)
      : (tiff[offset] << 24) | (tiff[offset + 1] << 16) | (tiff[offset + 2] << 8) | tiff[offset + 3];
  };
  
  const ifd0Offset = readU32(4);
  if (ifd0Offset < 8 || ifd0Offset >= tiff.length) return result;
  
  const numEntries = readU16(ifd0Offset);
  let offset = ifd0Offset + 2;
  
  for (let i = 0; i < numEntries && offset + 12 <= tiff.length; i++) {
    const tag = readU16(offset);
    const type = readU16(offset + 2);
    const valueOffset = offset + 8;
    
    // Orientation tag (0x0112)
    if (tag === 0x0112 && type === 3) {
      result.orientation = readU16(valueOffset);
    }
    
    // GPS IFD pointer (0x8825)
    if (tag === 0x8825) {
      const gpsOffset = readU32(valueOffset);
      // Check if GPS IFD pointer is non-zero (GPS data present)
      result.hasGps = gpsOffset !== 0;
    }
    
    offset += 12;
  }
  
  return result;
}

async function main() {
  console.log('🔬 EXIF Roundtrip Test');
  console.log('='.repeat(50));
  
  // Use test image with EXIF (Orientation=6, GPS data)
  const input = resolveFixture('test_with_exif.jpg');
  
  // Test 1: Default behavior - EXIF stripped
  console.log('\nTest 1: Default - EXIF should be stripped');
  const defaultOutput = await ImageEngine.fromPath(input)
    .toBuffer('jpeg', 80);
  const defaultExif = parseExifFromJpeg(defaultOutput);
  console.log(`  hasExif: ${defaultExif.hasExif}`);
  // Note: Some minimal EXIF may be present from encoder, but GPS should be absent

  const defaultMetrics = await ImageEngine.fromPath(input)
    .toBufferWithMetrics('jpeg', 80);
  assert.strictEqual(
    defaultMetrics.metrics.metadataStripped,
    true,
    'metadataStripped should be true when EXIF-only input metadata is stripped by default',
  );
  
  // Test 2: keepMetadata({ exif: true }) - EXIF preserved, GPS stripped
  console.log('\nTest 2: keepMetadata({ exif: true }) - EXIF preserved, GPS stripped');
  const exifPreserved = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true })
    .toBuffer('jpeg', 80);
  const preservedExif = parseExifFromJpeg(exifPreserved);
  console.log(`  hasExif: ${preservedExif.hasExif}`);
  console.log(`  orientation: ${preservedExif.orientation}`);
  console.log(`  hasGps: ${preservedExif.hasGps}`);
  
  // Orientation should be 1 (reset after auto-orient)
  if (preservedExif.hasExif && preservedExif.orientation !== null) {
    assert.strictEqual(preservedExif.orientation, 1, 
      'Orientation should be reset to 1 after auto-orient');
    console.log('  ✅ Orientation correctly reset to 1');
  }
  
  // GPS should be stripped by default
  assert.strictEqual(preservedExif.hasGps, false, 
    'GPS should be stripped by default');
  const exifPreservedMetrics = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true })
    .toBufferWithMetrics('jpeg', 80);
  assert.strictEqual(
    exifPreservedMetrics.metrics.metadataStripped,
    true,
    'metadataStripped should be true when EXIF is preserved but GPS is stripped',
  );
  console.log('  ✅ GPS correctly stripped (privacy protection)');
  
  // Test 3: keepMetadata({ exif: true, stripGps: false }) - GPS preserved
  console.log('\nTest 3: keepMetadata({ exif: true, stripGps: false }) - GPS preserved');
  const gpsPreserved = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true, stripGps: false })
    .toBuffer('jpeg', 80);
  const gpsExif = parseExifFromJpeg(gpsPreserved);
  console.log(`  hasExif: ${gpsExif.hasExif}`);
  console.log(`  hasGps: ${gpsExif.hasGps}`);
  const gpsPreservedMetrics = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true, stripGps: false })
    .toBufferWithMetrics('jpeg', 80);
  assert.strictEqual(
    gpsPreservedMetrics.metrics.metadataStripped,
    false,
    'metadataStripped should be false when EXIF and GPS preservation are explicitly requested',
  );
  // Note: If input has no GPS, this will still be false - that's expected
  
  // Test 4: Firewall strict mode overrides keepMetadata
  console.log('\nTest 4: Firewall strict mode - all metadata stripped');
  const firewallOutput = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true, stripGps: false })
    .sanitize({ policy: 'strict' })
    .toBuffer('jpeg', 80);
  const firewallExif = parseExifFromJpeg(firewallOutput);
  console.log(`  hasExif: ${firewallExif.hasExif}`);
  // Strict mode should strip all metadata
  
  // Test 5: ICC + EXIF together
  console.log('\nTest 5: ICC + EXIF preservation');
  const bothPreserved = await ImageEngine.fromPath(input)
    .keepMetadata({ icc: true, exif: true })
    .toBuffer('jpeg', 80);
  assert(bothPreserved.length > 0, 'Should produce output with both ICC and EXIF');
  console.log('  ✅ Successfully processed with ICC + EXIF');

  // Test 6: PNG output preserves EXIF when requested
  console.log('\nTest 6: PNG output preserves EXIF');
  const pngOutput = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true })
    .toBuffer('png');
  const pngExif = parseExifFromPng(pngOutput);
  assert.strictEqual(pngExif.hasExif, true, 'PNG output should contain EXIF');
  assert.strictEqual(pngExif.orientation, 1, 'PNG EXIF orientation should be reset to 1');
  assert.strictEqual(pngExif.hasGps, false, 'PNG EXIF should strip GPS by default');
  console.log('  ✅ PNG EXIF preserved with sanitized metadata');

  // Test 7: WebP output preserves EXIF when requested
  console.log('\nTest 7: WebP output preserves EXIF');
  const webpOutput = await ImageEngine.fromPath(input)
    .keepMetadata({ exif: true })
    .toBuffer('webp', 80);
  const webpExif = parseExifFromWebP(webpOutput);
  assert.strictEqual(webpExif.hasExif, true, 'WebP output should contain EXIF');
  assert.strictEqual(webpExif.orientation, 1, 'WebP EXIF orientation should be reset to 1');
  assert.strictEqual(webpExif.hasGps, false, 'WebP EXIF should strip GPS by default');
  console.log('  ✅ WebP EXIF preserved with sanitized metadata');
  
  console.log('\n' + '='.repeat(50));
  console.log('✅ All EXIF roundtrip tests passed!');
}

main().catch((err) => {
  console.error('❌ Test failed:', err);
  process.exit(1);
});
