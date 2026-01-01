/**
 * Create test fixtures with specific file sizes (100KB and 50MB)
 * Uses sharp to generate images from scratch
 */
const sharp = require('sharp');
const fs = require('fs');
const path = require('path');
const { resolveFixture } = require('./paths');

const TARGET_SIZES = {
  '100KB': 100 * 1024,      // 100KB
  '50MB': 50 * 1024 * 1024  // 50MB
};

const FORMATS = ['jpg', 'png', 'webp', 'avif'];

// Maximum dimensions to avoid encoder limits
const MAX_DIMENSIONS = {
  jpg: 32768,
  png: 32768,
  webp: 16383,  // WebP has 16383px limit
  avif: 32768
};

/**
 * Create a test image with complex patterns using sharp
 * For larger files, uses compositing to create larger file sizes
 */
async function createTestImage(width, height, format, quality, outputPath, targetSize) {
  // For very large files (>10MB), use compositing approach
  const isLargeFile = targetSize > 10 * 1024 * 1024;
  
  if (isLargeFile && width * height > 268435456) { // 16384^2 limit
    // Use compositing: create smaller tiles and composite them
    const tileSize = 8192; // Safe size for sharp
    const tilesX = Math.ceil(width / tileSize);
    const tilesY = Math.ceil(height / tileSize);
    
    // Create base image
    const baseSvg = `
      <svg width="${tileSize}" height="${tileSize}" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <linearGradient id="grad1" x1="0%" y1="0%" x2="100%" y2="100%">
            <stop offset="0%" style="stop-color:rgb(255,0,0);stop-opacity:1" />
            <stop offset="25%" style="stop-color:rgb(0,255,0);stop-opacity:1" />
            <stop offset="50%" style="stop-color:rgb(0,0,255);stop-opacity:1" />
            <stop offset="75%" style="stop-color:rgb(255,255,0);stop-opacity:1" />
            <stop offset="100%" style="stop-color:rgb(255,0,255);stop-opacity:1" />
          </linearGradient>
        </defs>
        <rect width="100%" height="100%" fill="url(#grad1)"/>
        ${Array.from({ length: 50 }, (_, i) => {
          const x = (i * tileSize) / 50;
          const y = (i * tileSize) / 50;
          return `<circle cx="${x}" cy="${y}" r="${tileSize / 100}" fill="rgba(${i * 5}, ${i * 3}, ${i * 7}, 0.8)"/>`;
        }).join('')}
      </svg>
    `;
    
    const baseImage = sharp(Buffer.from(baseSvg)).resize(tileSize, tileSize);
    const composite = [];
    
    for (let y = 0; y < tilesY; y++) {
      for (let x = 0; x < tilesX; x++) {
        composite.push({
          input: await baseImage.clone().toBuffer(),
          left: x * tileSize,
          top: y * tileSize
        });
      }
    }
    
    await sharp({
      create: {
        width: width,
        height: height,
        channels: 4,
        background: { r: 128, g: 128, b: 128, alpha: 1 }
      }
    })
      .composite(composite)
      .toFormat(format, getFormatOptions(format, quality))
      .toFile(outputPath);
    
    return;
  }
  
  // For smaller files, use SVG approach
  const complexity = isLargeFile ? 100 : 20;
  
  // Create a colorful test pattern with varying complexity
  const svg = `
    <svg width="${width}" height="${height}" xmlns="http://www.w3.org/2000/svg">
      <defs>
        <linearGradient id="grad1" x1="0%" y1="0%" x2="100%" y2="100%">
          <stop offset="0%" style="stop-color:rgb(255,0,0);stop-opacity:1" />
          <stop offset="25%" style="stop-color:rgb(0,255,0);stop-opacity:1" />
          <stop offset="50%" style="stop-color:rgb(0,0,255);stop-opacity:1" />
          <stop offset="75%" style="stop-color:rgb(255,255,0);stop-opacity:1" />
          <stop offset="100%" style="stop-color:rgb(255,0,255);stop-opacity:1" />
        </linearGradient>
        <pattern id="dots" x="0" y="0" width="20" height="20" patternUnits="userSpaceOnUse">
          <circle cx="10" cy="10" r="2" fill="rgba(0,0,0,0.3)"/>
        </pattern>
      </defs>
      <rect width="100%" height="100%" fill="url(#grad1)"/>
      <rect width="100%" height="100%" fill="url(#dots)"/>
      ${Array.from({ length: complexity }, (_, i) => {
        const x = (i * width) / complexity;
        const y = (i * height) / complexity;
        const radius = Math.min(width, height) / (complexity * 2);
        const hue = (i * 360) / complexity;
        return `<circle cx="${x}" cy="${y}" r="${radius}" fill="hsla(${hue}, 70%, 50%, 0.7)"/>`;
      }).join('')}
      ${Array.from({ length: Math.floor(complexity / 2) }, (_, i) => {
        const x1 = (i * width) / (complexity / 2);
        const y1 = 0;
        const x2 = width;
        const y2 = (i * height) / (complexity / 2);
        return `<line x1="${x1}" y1="${y1}" x2="${x2}" y2="${y2}" stroke="rgba(255,255,255,0.3)" stroke-width="2"/>`;
      }).join('')}
    </svg>
  `;

  let sharpInstance = sharp(Buffer.from(svg));

  await sharpInstance
    .resize(width, height, { fit: 'fill' })
    .toFormat(format, getFormatOptions(format, quality))
    .toFile(outputPath);
}

function getFormatOptions(format, quality) {
  const options = {};
  if (format === 'jpg' || format === 'jpeg') {
    options.quality = quality;
    options.mozjpeg = true;
  } else if (format === 'png') {
    options.quality = quality;
    options.compressionLevel = 9;
  } else if (format === 'webp') {
    options.quality = quality;
  } else if (format === 'avif') {
    options.quality = quality;
  }
  return options;
}

/**
 * Generate image with target file size by adjusting dimensions
 */
async function createFixtureWithSize(outputPath, format, targetSize) {
  console.log(`Creating ${path.basename(outputPath)} (target: ${(targetSize / 1024).toFixed(0)}KB)...`);
  
  // Start with reasonable dimensions based on target size
  let width, height;
  let quality;
  
  if (targetSize < 1024 * 1024) {
    // For smaller files (< 1MB), start smaller
    width = 1000;
    height = 1000;
  } else {
    // For larger files, start larger
    width = 5000;
    height = 5000;
  }
  
  // Quality settings based on format and target size
  if (format === 'avif') {
    quality = targetSize < 1024 * 1024 ? 90 : 100; // Higher quality for larger files
  } else if (format === 'webp') {
    quality = targetSize < 1024 * 1024 ? 80 : 90;
  } else if (format === 'jpg' || format === 'jpeg') {
    quality = targetSize < 1024 * 1024 ? 85 : 95;
  } else {
    quality = 100; // PNG
  }
  
  let currentSize = 0;
  let attempts = 0;
  const maxAttempts = 20;
  const maxDim = MAX_DIMENSIONS[format];
  
  // Binary search approach to find right dimensions
  let minWidth = 100;
  let maxWidth = maxDim;
  
  while (attempts < maxAttempts) {
    // Clamp dimensions to max
    width = Math.min(width, maxDim);
    height = Math.min(height, maxDim);
    
    try {
      await createTestImage(width, height, format, quality, outputPath, targetSize);
      const stats = fs.statSync(outputPath);
      currentSize = stats.size;
      const ratio = targetSize / currentSize;
      
      console.log(`  Attempt ${attempts + 1}: ${width}x${height}, quality ${quality} -> ${(currentSize / 1024).toFixed(0)}KB (target: ${(targetSize / 1024).toFixed(0)}KB)`);
      
      // For very large files, accept 5% tolerance; for smaller files, 10%
      const tolerance = targetSize > 10 * 1024 * 1024 ? 0.05 : 0.1;
      
      if (Math.abs(currentSize - targetSize) / targetSize < tolerance) {
        // Within tolerance, good enough
        console.log(`  ✅ Created ${path.basename(outputPath)}: ${(currentSize / 1024).toFixed(0)}KB\n`);
        return;
      }
      
      if (currentSize < targetSize) {
        // Too small, increase dimensions
        minWidth = width;
        const scale = Math.sqrt(ratio);
        width = Math.min(Math.floor(width * scale), maxWidth);
        // Maintain aspect ratio
        const aspectRatio = height / (width / scale);
        height = Math.floor(width * aspectRatio);
        height = Math.min(height, maxDim);
      } else {
        // Too large, decrease dimensions
        maxWidth = width;
        const scale = Math.sqrt(ratio);
        width = Math.max(Math.floor(width / scale), minWidth);
        // Maintain aspect ratio
        const aspectRatio = height / (width * scale);
        height = Math.floor(width * aspectRatio);
      }
      
      // Adjust quality for fine-tuning (only for smaller adjustments)
      if (attempts > 10 && Math.abs(ratio - 1) < 0.5) {
        if (currentSize < targetSize && quality < 100) {
          quality = Math.min(quality + 1, 100);
        } else if (currentSize > targetSize && quality > 20) {
          quality = Math.max(quality - 1, 20);
        }
      }
      
      attempts++;
    } catch (error) {
      // If we hit dimension limits, try reducing size
      if (error.message.includes('dimension') || error.message.includes('DIMENSION')) {
        console.log(`  ⚠️  Dimension limit hit, reducing size...`);
        width = Math.floor(width * 0.8);
        height = Math.floor(height * 0.8);
        attempts++;
        continue;
      }
      // For other errors, try with lower quality
      if (attempts < 3) {
        quality = Math.max(quality - 10, 20);
        attempts++;
        continue;
      }
      throw error;
    }
  }
  
  // Final attempt - use what we have if close enough
  try {
    width = Math.min(width, maxDim);
    height = Math.min(height, maxDim);
    await createTestImage(width, height, format, quality, outputPath, targetSize);
    const stats = fs.statSync(outputPath);
    const finalSize = stats.size;
    console.log(`  ✅ Created ${path.basename(outputPath)}: ${(finalSize / 1024).toFixed(0)}KB (target: ${(targetSize / 1024).toFixed(0)}KB, ${((finalSize / targetSize - 1) * 100).toFixed(1)}% diff)\n`);
  } catch (error) {
    throw error;
  }
}

async function main() {
  console.log('Creating test fixtures with specific file sizes using sharp...\n');
  
  for (const [sizeName, targetSize] of Object.entries(TARGET_SIZES)) {
    for (const format of FORMATS) {
      const filename = `test_${sizeName}.${format}`;
      const outputPath = resolveFixture(filename);
      
      try {
        await createFixtureWithSize(outputPath, format, targetSize);
      } catch (error) {
        console.error(`  ❌ Failed to create ${filename}: ${error.message}\n`);
      }
    }
  }
  
  console.log('Done!');
  console.log('\nCreated files:');
  for (const [sizeName] of Object.entries(TARGET_SIZES)) {
    for (const format of FORMATS) {
      const filename = `test_${sizeName}.${format}`;
      const filePath = resolveFixture(filename);
      if (fs.existsSync(filePath)) {
        const stats = fs.statSync(filePath);
        console.log(`  ${filename}: ${(stats.size / 1024).toFixed(0)}KB`);
      }
    }
  }
}

if (require.main === module) {
  main().catch(console.error);
}

module.exports = { main, createFixtureWithSize, createTestImage };

