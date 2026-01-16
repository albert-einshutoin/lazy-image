/**
 * Create small, valid JPEG/PNG fixtures for CI environments.
 * Uses embedded base64 to avoid external dependencies.
 */
const fs = require('fs');
const path = require('path');
const { resolveFixture } = require('./paths');

// 1x1 white JPEG (valid JFIF with EOI marker) and PNG
const JPEG_BASE64 =
  '/9j/2wBDAAYEBQYFBAYGBQYHBwYIChAKCgkJChQODwwQFxQYGBcUFhYaHSUfGhsjHBYWICwgIyYnKSopGR8tMC0oMCUoKSj/2wBDAQcHBwoIChMKChMoGhYaKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCgoKCj/wAARCAABAAEDASIAAhEBAxEB/8QAFQABAQAAAAAAAAAAAAAAAAAAAAj/xAAUEAEAAAAAAAAAAAAAAAAAAAAA/8QAFAEBAAAAAAAAAAAAAAAAAAAAAP/EABQRAQAAAAAAAAAAAAAAAAAAAAD/2gAMAwEAAhEDEQA/AKpAB//Z';

const PNG_BASE64 =
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAuMBgX9n1r0AAAAASUVORK5CYII=';

function writeFixture(filename, base64) {
  const outputPath = resolveFixture(filename);
  const buffer = Buffer.from(base64, 'base64');
  fs.mkdirSync(path.dirname(outputPath), { recursive: true });
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

module.exports = { main, writeFixture };
