/**
 * Create a minimal test image for CI environments.
 * Uses sharp (devDependency) to create a valid JPEG.
 */
const fs = require('fs');
const path = require('path');

async function createTestImage() {
    try {
        const sharp = require('sharp');
        
        // Create a 200x200 gradient image
        const width = 200;
        const height = 200;
        const channels = 3;
        const pixels = Buffer.alloc(width * height * channels);
        
        // Create a simple gradient pattern
        for (let y = 0; y < height; y++) {
            for (let x = 0; x < width; x++) {
                const idx = (y * width + x) * channels;
                pixels[idx] = Math.floor((x / width) * 255);     // R
                pixels[idx + 1] = Math.floor((y / height) * 255); // G
                pixels[idx + 2] = 128;                            // B
            }
        }
        
        // Create JPEG using sharp
        const jpeg = await sharp(pixels, {
            raw: { width, height, channels }
        }).jpeg({ quality: 90 }).toBuffer();
        
        const outputPath = path.join(__dirname, '..', 'test_input.jpg');
        fs.writeFileSync(outputPath, jpeg);
        console.log(`Created test image: ${outputPath} (${jpeg.length} bytes)`);
        
    } catch (e) {
        console.error('Failed to create test image:', e.message);
        console.log('sharp is required as a devDependency to create test images');
        process.exit(1);
    }
}

createTestImage();
