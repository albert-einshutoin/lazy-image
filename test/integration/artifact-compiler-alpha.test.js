const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');
const sharp = require('sharp');
const { ImageEngine, compileImage } = require('../../index');
const { createRgbaPng } = require('../helpers/png-helpers');

async function main() {
  const parent = await fs.mkdtemp(path.join(os.tmpdir(), 'lazy-image-alpha-'));
  try {
    for (const [format, alpha, icc] of [['png', 128, false], ['webp', 128, false], ['png', 0, false], ['png', 255, false], ['png', 128, true]]) {
      const inputPath = path.join(parent, `input-${alpha}-${icc}.${format}`);
      let png = createRgbaPng(32, 24, [255, 0, 0, alpha]);
      if (icc) {
        png = await sharp(png).withIccProfile('srgb').png().toBuffer();
      }
      await fs.writeFile(inputPath, format === 'png' ? png : await ImageEngine.from(png).toBuffer('webp', 80, false));
      const outputDir = path.join(parent, `output-${format}-${alpha}-${icc}`);
      const manifest = await compileImage({ inputPath, outputDir,
        policy: { widths: [32], formats: icc ? ['avif'] : ['webp', 'avif'], placeholder: false } });
      assert.equal(manifest.artifacts.length, icc ? 1 : 2);
      for (const artifact of manifest.artifacts) {
        const { data, info } = await sharp(path.join(outputDir, artifact.path)).raw().toBuffer({ resolveWithObject: true })
          .catch((error) => { throw new Error(`${format}/${alpha}/${icc}/${artifact.format}: ${error.message}`); });
        assert.equal(info.channels, alpha === 255 ? 3 : 4);
        assert.equal(artifact.iccOutcome, icc ? 'preserved' : 'absent');
        assert.equal(info.width, 32);
        assert.equal(info.height, 24);
        if (alpha !== 255) for (let i = 3; i < data.length; i += 4) assert.ok(Math.abs(data[i] - alpha) <= 1);
      }
    }

    const inputPath = path.join(parent, 'input-128-false.png');
    const mutations = [
      ['alpha-item-id', 'infe', 1, (b, o) => b.writeUInt16BE(3, o + 8)],
      ['alpha-item-type', 'infe', 1, (b, o) => b.write('Exif', o + 12)],
      ['alpha-name', 'infe', 1, (b, o) => b.write('GPS!!', o + 16)],
      ['aux-target', 'auxl', 0, (b, o) => b.writeUInt16BE(2, o + 8)],
      ['aux-source', 'auxl', 0, (b, o) => b.writeUInt16BE(1, o + 4)],
      ['aux-property', 'auxC', 0, (b, o) => { b[o + 10] ^= 1; }],
      ['alpha-channels', 'pixi', 1, (b, o) => { b[o + 8] = 3; }],
      ['alpha-config', 'av1C', 1, (b, o) => { b[o + 6] = 0x0c; }],
      ['alpha-association', 'ipma', 0, (b, o) => { b[o + 24] = 0x83; }],
      ['extent-swap', 'iloc', 0, (b, o) => {
        const color = Buffer.from(b.subarray(o + 18, o + 26));
        b.copy(b, o + 18, o + 32, o + 40);
        color.copy(b, o + 32);
      }],
      ['extent-overlap', 'iloc', 0, (b, o) => b.writeUInt32BE(b.readUInt32BE(o + 18), o + 32)],
      ['extent-gap', 'iloc', 0, (b, o) => b.writeUInt32BE(b.readUInt32BE(o + 32) + 1, o + 32)],
      ['extent-oob', 'iloc', 0, (b, o) => b.writeUInt32BE(0xffffffff, o + 32)],
      ['extent-length', 'iloc', 0, (b, o) => b.writeUInt32BE(0xffffffff, o + 36)],
      ['item-count', 'iinf', 0, (b, o) => b.writeUInt16BE(1, o + 8)],
      ['unknown-box', 'auxC', 0, (b, o) => b.write('junk', o)],
      ['truncated-box', 'auxC', 0, (b, o) => b.writeUInt32BE(7, o - 4)],
      ['missing-reference', 'iref', 0, (b, o) => b.write('iprp', o)],
    ];
    for (const [name, type, occurrence, mutate] of mutations) {
      const outputDir = path.join(parent, name);
      const original = ImageEngine.prototype.toFileWithMetrics;
      let injected = false;
      ImageEngine.prototype.toFileWithMetrics = async function (...args) {
        const result = await original.apply(this, args);
        if (args[1] === 'avif') {
          const data = await fs.readFile(args[0]);
          let offset = -1;
          for (let i = 0; i <= occurrence; i += 1) offset = data.indexOf(Buffer.from(type), offset + 1);
          assert.ok(offset >= 0, name);
          mutate(data, offset);
          await fs.writeFile(args[0], data);
          injected = true;
        }
        return result;
      };
      try {
        await assert.rejects(() => compileImage({ inputPath, outputDir,
          policy: { widths: [32], formats: ['webp', 'avif'], placeholder: false } }),
        (error) => error.phase === 'verification' && error.errorCode === 'E900', name);
        assert.ok(injected, name);
        assert.equal(await fs.stat(outputDir).catch(() => null), null, name);
        assert.deepEqual((await fs.readdir(parent)).filter((entry) => entry.includes('staging')), [], name);
      } finally {
        ImageEngine.prototype.toFileWithMetrics = original;
      }
    }
  } finally {
    await fs.rm(parent, { recursive: true, force: true });
  }
}
main().then(() => console.log('artifact compiler alpha contract passed'), (error) => {
  console.error(error);
  process.exitCode = 1;
});
