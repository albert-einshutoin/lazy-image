/**
 * Create a test JPEG with EXIF data including GPS coordinates
 * Run this script to generate test/fixtures/test_with_exif.jpg
 */

const fs = require('fs');
const path = require('path');

// Minimal 1x1 red JPEG with EXIF including Orientation=6 and GPS data
function createJpegWithExif() {
  const parts = [];
  
  // SOI
  parts.push(Buffer.from([0xFF, 0xD8]));
  
  // APP1 (EXIF) segment
  const exifData = createExifData();
  const app1 = Buffer.alloc(4 + exifData.length);
  app1[0] = 0xFF;
  app1[1] = 0xE1;
  const len = exifData.length + 2;
  app1[2] = (len >> 8) & 0xFF;
  app1[3] = len & 0xFF;
  exifData.copy(app1, 4);
  parts.push(app1);
  
  // DQT
  parts.push(Buffer.from([
    0xFF, 0xDB, 0x00, 0x43, 0x00,
    0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08, 0x07,
    0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14,
    0x0D, 0x0C, 0x0B, 0x0B, 0x0C, 0x19, 0x12, 0x13,
    0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A,
    0x1C, 0x1C, 0x20, 0x24, 0x2E, 0x27, 0x20, 0x22,
    0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C,
    0x30, 0x31, 0x34, 0x34, 0x34, 0x1F, 0x27, 0x39,
    0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32,
  ]));
  
  // SOF0 (8x8 for visibility)
  parts.push(Buffer.from([
    0xFF, 0xC0, 0x00, 0x0B, 0x08,
    0x00, 0x08, // height = 8
    0x00, 0x08, // width = 8
    0x01, // 1 component (grayscale)
    0x01, 0x11, 0x00,
  ]));
  
  // DHT
  parts.push(Buffer.from([
    0xFF, 0xC4, 0x00, 0x1F, 0x00,
    0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01,
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
    0x08, 0x09, 0x0A, 0x0B,
  ]));
  
  // SOS + minimal scan data
  parts.push(Buffer.from([
    0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00,
    0x7F, 0xFF, 0xD9 // Minimal data + EOI
  ]));
  
  return Buffer.concat(parts);
}

function createExifData() {
  // Exif header + TIFF structure
  const parts = [];
  
  // "Exif\0\0" header
  parts.push(Buffer.from('Exif\0\0'));
  
  // TIFF header (little-endian)
  const tiffStart = 6; // offset after Exif header
  const tiff = [];
  
  // Byte order: II (little-endian)
  tiff.push(0x49, 0x49);
  // Magic number: 42
  tiff.push(0x2A, 0x00);
  // IFD0 offset (8 bytes from TIFF start)
  tiff.push(0x08, 0x00, 0x00, 0x00);
  
  // IFD0 entries
  const ifd0Entries = [
    // Orientation tag (0x0112) = 6 (rotated 90 CW)
    createIfdEntry(0x0112, 3, 1, 6),
    // GPS IFD pointer (0x8825) - points to GPS IFD
    createIfdEntry(0x8825, 4, 1, 0), // Will update offset later
  ];
  
  // Number of IFD0 entries
  tiff.push(ifd0Entries.length & 0xFF, (ifd0Entries.length >> 8) & 0xFF);
  
  // Calculate GPS IFD offset
  const ifd0Size = 2 + (ifd0Entries.length * 12) + 4; // count + entries + next IFD pointer
  const gpsIfdOffset = 8 + ifd0Size; // TIFF header + IFD0
  
  // Update GPS IFD pointer
  ifd0Entries[1] = createIfdEntry(0x8825, 4, 1, gpsIfdOffset);
  
  // Add IFD0 entries
  for (const entry of ifd0Entries) {
    tiff.push(...entry);
  }
  
  // Next IFD pointer (0 = no more IFDs)
  tiff.push(0x00, 0x00, 0x00, 0x00);
  
  // GPS IFD
  const gpsEntries = [
    // GPS Latitude Ref (0x0001) = "N"
    createIfdEntry(0x0001, 2, 2, 0x004E), // 'N\0'
    // GPS Latitude (0x0002) - simplified, just use offset
    createIfdEntry(0x0002, 5, 3, gpsIfdOffset + 50), // Will point to rational data
  ];
  
  // Number of GPS IFD entries
  tiff.push(gpsEntries.length & 0xFF, (gpsEntries.length >> 8) & 0xFF);
  
  // Add GPS IFD entries
  for (const entry of gpsEntries) {
    tiff.push(...entry);
  }
  
  // Next IFD pointer
  tiff.push(0x00, 0x00, 0x00, 0x00);
  
  // GPS Latitude data (3 rationals: degrees, minutes, seconds)
  // 35°40'30" N = Tokyo area
  // Each rational is 8 bytes (numerator + denominator)
  tiff.push(
    0x23, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // 35/1
    0x28, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // 40/1
    0x1E, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, // 30/1
  );
  
  parts.push(Buffer.from(tiff));
  
  return Buffer.concat(parts);
}

function createIfdEntry(tag, type, count, value) {
  return [
    tag & 0xFF, (tag >> 8) & 0xFF,           // Tag
    type & 0xFF, (type >> 8) & 0xFF,         // Type
    count & 0xFF, (count >> 8) & 0xFF, 0, 0, // Count
    value & 0xFF, (value >> 8) & 0xFF, (value >> 16) & 0xFF, (value >> 24) & 0xFF, // Value/Offset
  ];
}

// Generate and save
const jpeg = createJpegWithExif();
const outputPath = path.join(__dirname, 'test_with_exif.jpg');
fs.writeFileSync(outputPath, jpeg);
console.log(`Created ${outputPath} (${jpeg.length} bytes)`);

// Verify
const data = fs.readFileSync(outputPath);
let i = 2;
while (i + 10 < data.length) {
  if (data[i] === 0xFF && data[i + 1] === 0xE1) {
    const header = data.slice(i + 4, i + 10).toString();
    console.log('EXIF header found:', header.startsWith('Exif') ? 'YES' : 'NO');
    break;
  }
  i++;
}
