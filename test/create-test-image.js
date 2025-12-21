/**
 * Create small, valid JPEG/PNG fixtures for CI environments.
 * Uses embedded base64 to avoid external dependencies.
 */
const fs = require('fs');
const path = require('path');

// 1x1 white JPEG (valid JFIF) and PNG
const JPEG_BASE64 =
  '/9j/4AAQSkZJRgABAQAAAQABAAD/2wBDAP//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////2wBDAf//////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////wAARCAAQABADASIAAhEBAxEB/8QAFwABAQEBAAAAAAAAAAAAAAAAAAUGB//EABwQAAICAgMAAAAAAAAAAAAAAAABAhEDBBIhMf/EABUBAQEAAAAAAAAAAAAAAAAAAAAB/8QAFhEAAwAAAAAAAAAAAAAAAAAAAAER/9oADAMBAAIRAxEAPwCmgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA//2Q==';

const PNG_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAuMBgX9n1r0AAAAASUVORK5CYII=';

function writeFixture(filename, base64) {
  const outputPath = path.join(__dirname, '..', filename);
  const buffer = Buffer.from(base64, 'base64');
  fs.writeFileSync(outputPath, buffer);
  console.log(`Created ${filename}: ${buffer.length} bytes`);
}

function main() {
  writeFixture('test_input.jpg', JPEG_BASE64);
  writeFixture('test_input.png', PNG_BASE64);
}

if (require.main === module) {
  main();
}
