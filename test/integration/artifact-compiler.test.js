const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const fs = require('node:fs');
const fsp = require('node:fs/promises');
const os = require('node:os');
const path = require('node:path');

const { ImageEngine, compileImage } = require('../../index');
const { compilerFingerprint } = require('../../lib/artifact-compiler');
const { resolveFixture } = require('../helpers/paths');

const INPUT = resolveFixture('test_100KB_1057x1057.jpg');

async function withTempParent(run) {
  const parent = await fsp.mkdtemp(path.join(os.tmpdir(), 'lazy-image-compiler-'));
  try {
    return await run(parent);
  } finally {
    await fsp.rm(parent, { recursive: true, force: true });
  }
}

async function hashFile(filePath) {
  const hash = crypto.createHash('sha256');
  for await (const chunk of fs.createReadStream(filePath)) hash.update(chunk);
  return hash.digest('hex');
}

async function assertRejected(run, check) {
  await assert.rejects(run, (error) => {
    check(error);
    return true;
  });
}

async function writeUnknownOrientationFixture(format, outputPath) {
  const data = await ImageEngine.fromPath(resolveFixture('test_with_exif.jpg'))
    .keepMetadata({ icc: false, exif: true, stripGps: false })
    .toBuffer(format, 80, false);
  const orientationTag = Buffer.from([0x12, 0x01, 0x03, 0x00]);
  const tagOffset = data.indexOf(orientationTag);
  assert.notEqual(tagOffset, -1, `${format} fixture should contain EXIF orientation`);
  data[tagOffset + 8] = 9;
  data[tagOffset + 9] = 0;
  await fsp.writeFile(outputPath, data);
}

function makeIccProfile(shared) {
  const profile = Buffer.alloc(shared ? 164 : 132);
  profile.writeUInt32BE(profile.length, 0);
  profile.write('ADBE', 4, 'ascii');
  profile[8] = 2;
  profile.write('mntr', 12, 'ascii');
  profile.write('RGB ', 16, 'ascii');
  profile.write('XYZ ', 20, 'ascii');
  profile.write('acsp', 36, 'ascii');
  if (shared) {
    profile.writeUInt32BE(2, 128);
    profile.write('rXYZ', 132, 'ascii');
    profile.writeUInt32BE(156, 136);
    profile.writeUInt32BE(8, 140);
    profile.write('gXYZ', 144, 'ascii');
    profile.writeUInt32BE(156, 148);
    profile.writeUInt32BE(8, 152);
    profile.write('curv', 156, 'ascii');
  }
  return profile;
}

async function writeIccFixture(outputPath, { shared, split }) {
  const base = await ImageEngine.fromPath(INPUT).toBuffer('jpeg', 80, false);
  const profile = makeIccProfile(shared);
  const parts = split ? [profile.subarray(0, 80), profile.subarray(80)] : [profile];
  const segments = parts.map((part, index) => {
    const payload = Buffer.concat([
      Buffer.from('ICC_PROFILE\0', 'ascii'),
      Buffer.from([index + 1, parts.length]),
      part,
    ]);
    const segment = Buffer.concat([Buffer.from([0xff, 0xe2, 0, 0]), payload]);
    segment.writeUInt16BE(segment.length - 2, 2);
    return segment;
  });
  const sos = base.indexOf(Buffer.from([0xff, 0xda]));
  await fsp.writeFile(outputPath, Buffer.concat([base.subarray(0, sos), ...segments, base.subarray(sos)]));
}

async function main() {
  const esm = await import('../../index.mjs');
  assert.equal(typeof esm.compileImage, 'function');
  assert.notEqual(
    compilerFingerprint('1.0.1', { build: 'codec-a' }),
    compilerFingerprint('1.0.1', { build: 'codec-b' }),
    'compiler build identity must participate in the cache fingerprint',
  );

  await withTempParent(async (parent) => {
    const outputDir = path.join(parent, 'published');
    const manifest = await compileImage({
      inputPath: INPUT,
      outputDir,
      policy: { widths: [320], formats: ['webp'], placeholder: true },
    });

    assert.equal(manifest.schemaVersion, 1);
    assert.deepEqual(manifest.policy.widths, [320]);
    assert.deepEqual(manifest.policy.formats, ['webp']);
    assert.equal(manifest.artifacts.length, 1);
    assert.deepEqual(Object.keys(manifest.artifacts[0]), [
      'id', 'path', 'format', 'width', 'height', 'bytes', 'quality', 'iccOutcome', 'sha256',
    ]);
    assert.equal(manifest.artifacts[0].path, 'image-320.webp');
    assert.equal(manifest.placeholder.path, 'placeholder.webp');
    assert.match(manifest.placeholder.dataUrl, /^data:image\/webp;base64,/);
    assert.ok(!JSON.stringify(manifest).includes(INPUT));
    assert.ok(!JSON.stringify(manifest).includes(path.basename(INPUT)));

    const manifestBytes = await fsp.readFile(path.join(outputDir, 'manifest.json'), 'utf8');
    assert.deepEqual(JSON.parse(manifestBytes), manifest);
    assert.equal(manifest.artifacts[0].bytes, (await fsp.stat(path.join(outputDir, manifest.artifacts[0].path))).size);
    assert.equal(manifest.artifacts[0].sha256, await hashFile(path.join(outputDir, manifest.artifacts[0].path)));
    assert.deepEqual((await fsp.readdir(outputDir)).filter((name) => !name.startsWith('.')).sort(), [
      'image-320.webp', 'manifest.json', 'placeholder.webp',
    ]);
  });

  await withTempParent(async (parent) => {
    const outputDir = path.join(parent, 'already-there');
    await fsp.mkdir(outputDir);
    await assertRejected(
      () => compileImage({ inputPath: INPUT, outputDir, policy: { widths: [320], formats: ['webp'] } }),
      (error) => {
        assert.equal(error.name, 'ArtifactCompilationError');
        assert.equal(error.phase, 'commit');
        assert.equal(error.errorCode, 'E301');
      },
    );
    assert.deepEqual(await fsp.readdir(outputDir), []);
  });

  for (const fixture of [{ shared: false, split: false }, { shared: true, split: true }]) {
    await withTempParent(async (parent) => {
      const inputPath = path.join(parent, 'icc-input.jpg');
      await writeIccFixture(inputPath, fixture);
      const manifest = await compileImage({
        inputPath,
        outputDir: path.join(parent, 'icc-output'),
        policy: { widths: [320], formats: ['jpeg'], placeholder: false },
      });
      assert.equal(manifest.artifacts[0].iccOutcome, 'preserved');
    });
  }

  await withTempParent(async (parent) => {
    await assertRejected(
      () => compileImage({
        inputPath: INPUT,
        outputDir: path.join(parent, 'invalid-policy'),
        policy: { unknown: true },
      }),
      (error) => {
        assert.equal(error.name, 'ArtifactCompilationError');
        assert.equal(error.phase, 'planning');
        assert.equal(error.errorCode, 'E400');
        assert.equal(error.category, 0);
      },
    );
  });

  await withTempParent(async (parent) => {
    const outputDir = path.join(parent, 'budget-failure');
    await assertRejected(
      () => compileImage({
        inputPath: INPUT,
        outputDir,
        policy: {
          widths: [320],
          formats: ['webp'],
          budgets: [{ width: 320, format: 'webp', maxBytes: 1 }],
          placeholder: false,
        },
      }),
      (error) => {
        assert.equal(error.name, 'ArtifactCompilationError');
        assert.equal(error.phase, 'processing');
        assert.equal(error.artifactId, '320/webp');
        assert.equal(error.errorCode, 'E300');
      },
    );
    assert.equal(await fsp.stat(outputDir).catch(() => null), null);
  });

  await withTempParent(async (parent) => {
    const outputDir = path.join(parent, 'aborted');
    const controller = new AbortController();
    controller.abort();
    await assertRejected(
      () => compileImage({ inputPath: INPUT, outputDir, policy: { widths: [320], formats: ['webp'] }, signal: controller.signal }),
      (error) => {
        assert.equal(error.name, 'AbortError');
        assert.equal(error.phase, 'preflight');
      },
    );
    assert.equal(await fsp.stat(outputDir).catch(() => null), null);
  });

  for (const format of ['png', 'webp']) {
    await withTempParent(async (parent) => {
      const inputPath = path.join(parent, `unknown-orientation.${format}`);
      await writeUnknownOrientationFixture(format, inputPath);
      await assertRejected(
        () => compileImage({
          inputPath,
          outputDir: path.join(parent, 'orientation-output'),
          policy: { widths: [16], formats: ['webp'], placeholder: false },
        }),
        (error) => {
          assert.equal(error.name, 'ArtifactCompilationError');
          assert.equal(error.phase, 'preflight');
          assert.equal(error.errorCode, 'E130');
        },
      );
    });
  }

  await withTempParent(async (parent) => {
    const inputPath = path.join(parent, 'mutable-input.jpg');
    const original = await fsp.readFile(INPUT);
    const mutated = Buffer.from(original);
    mutated[mutated.length - 1] ^= 1;
    await fsp.writeFile(inputPath, original);
    const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
    let changed = false;
    ImageEngine.prototype.toFileWithMetrics = async function patchedToFileWithMetrics(...args) {
      const result = await originalToFileWithMetrics.apply(this, args);
      if (!changed) {
        changed = true;
        await fsp.writeFile(inputPath, mutated);
      }
      return result;
    };
    try {
      await assertRejected(
        () => compileImage({
          inputPath,
          outputDir: path.join(parent, 'mutable-output'),
          policy: { widths: [320, 640], formats: ['webp'], placeholder: false },
        }),
        (error) => {
          assert.equal(error.name, 'ArtifactCompilationError');
          assert.equal(error.errorCode, 'E101');
        },
      );
    } finally {
      ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
    }
    assert.equal(await fsp.stat(path.join(parent, 'mutable-output')).catch(() => null), null);
  });

  await withTempParent(async (parent) => {
    const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
    let injected = false;
    ImageEngine.prototype.toFileWithMetrics = async function patchedAvifMetadata(...args) {
      const result = await originalToFileWithMetrics.apply(this, args);
      if (!injected && args[1] === 'avif') {
        injected = true;
        const uuid = Buffer.alloc(12);
        uuid.writeUInt32BE(uuid.length, 0);
        uuid.write('uuid', 4, 'ascii');
        await fsp.appendFile(args[0], uuid);
        return {
          ...result,
          bytesWritten: result.bytesWritten + uuid.length,
          metrics: { ...result.metrics, bytesOut: result.metrics.bytesOut + uuid.length },
        };
      }
      return result;
    };
    try {
      await assertRejected(
        () => compileImage({
          inputPath: INPUT,
          outputDir: path.join(parent, 'avif-metadata-output'),
          policy: { widths: [320], formats: ['avif'], placeholder: false },
        }),
        (error) => {
          assert.equal(error.name, 'ArtifactCompilationError');
          assert.equal(error.phase, 'verification');
          assert.equal(error.errorCode, 'E900');
        },
      );
    } finally {
      ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
    }
    assert.equal(await fsp.stat(path.join(parent, 'avif-metadata-output')).catch(() => null), null);
  });

  await withTempParent(async (parent) => {
    const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
    let injected = false;
    ImageEngine.prototype.toFileWithMetrics = async function patchedAvifUnknownBox(...args) {
      const result = await originalToFileWithMetrics.apply(this, args);
      if (!injected && args[1] === 'avif') {
        injected = true;
        const junk = Buffer.alloc(12);
        junk.writeUInt32BE(junk.length, 0);
        junk.write('junk', 4, 'ascii');
        await fsp.appendFile(args[0], junk);
        return {
          ...result,
          bytesWritten: result.bytesWritten + junk.length,
          metrics: { ...result.metrics, bytesOut: result.metrics.bytesOut + junk.length },
        };
      }
      return result;
    };
    try {
      await assertRejected(
        () => compileImage({
          inputPath: INPUT,
          outputDir: path.join(parent, 'avif-unknown-box-output'),
          policy: { widths: [320], formats: ['avif'], placeholder: false },
        }),
        (error) => {
          assert.equal(error.name, 'ArtifactCompilationError');
          assert.equal(error.phase, 'verification');
          assert.equal(error.errorCode, 'E900');
        },
      );
    } finally {
      ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
    }
    assert.equal(await fsp.stat(path.join(parent, 'avif-unknown-box-output')).catch(() => null), null);
  });

  await withTempParent(async (parent) => {
    const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
    let injected = false;
    ImageEngine.prototype.toFileWithMetrics = async function patchedJpegMetadataAfterScan(...args) {
      const result = await originalToFileWithMetrics.apply(this, args);
      if (!injected && args[1] === 'jpeg') {
        injected = true;
        const data = await fsp.readFile(args[0]);
        const eoi = data.lastIndexOf(Buffer.from([0xff, 0xd9]));
        assert.ok(eoi > 0);
        const exif = Buffer.from([0xff, 0xe1, 0x00, 0x08, 0x45, 0x78, 0x69, 0x66, 0x00, 0x00]);
        await fsp.writeFile(args[0], Buffer.concat([data.subarray(0, eoi), exif, data.subarray(eoi)]));
        return {
          ...result,
          bytesWritten: result.bytesWritten + exif.length,
          metrics: { ...result.metrics, bytesOut: result.metrics.bytesOut + exif.length },
        };
      }
      return result;
    };
    try {
      await assertRejected(
        () => compileImage({
          inputPath: INPUT,
          outputDir: path.join(parent, 'jpeg-post-scan-output'),
          policy: { widths: [320], formats: ['jpeg'], placeholder: false },
        }),
        (error) => {
          assert.equal(error.name, 'ArtifactCompilationError');
          assert.equal(error.phase, 'verification');
          assert.equal(error.errorCode, 'E900');
        },
      );
    } finally {
      ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
    }
    assert.equal(await fsp.stat(path.join(parent, 'jpeg-post-scan-output')).catch(() => null), null);
  });

  await withTempParent(async (parent) => {
    const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
    let injected = false;
    ImageEngine.prototype.toFileWithMetrics = async function patchedJpegTrailingData(...args) {
      const result = await originalToFileWithMetrics.apply(this, args);
      if (!injected && args[1] === 'jpeg') {
        injected = true;
        await fsp.appendFile(args[0], Buffer.from([0x00]));
        return {
          ...result,
          bytesWritten: result.bytesWritten + 1,
          metrics: { ...result.metrics, bytesOut: result.metrics.bytesOut + 1 },
        };
      }
      return result;
    };
    try {
      await assertRejected(
        () => compileImage({
          inputPath: INPUT,
          outputDir: path.join(parent, 'jpeg-trailing-output'),
          policy: { widths: [320], formats: ['jpeg'], placeholder: false },
        }),
        (error) => {
          assert.equal(error.name, 'ArtifactCompilationError');
          assert.equal(error.phase, 'verification');
          assert.equal(error.errorCode, 'E900');
        },
      );
    } finally {
      ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
    }
    assert.equal(await fsp.stat(path.join(parent, 'jpeg-trailing-output')).catch(() => null), null);
  });

  for (const format of ['jpeg', 'webp', 'avif']) {
    await withTempParent(async (parent) => {
      const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
      let injected = false;
      ImageEngine.prototype.toFileWithMetrics = async function patchedMalformedIcc(...args) {
        const result = await originalToFileWithMetrics.apply(this, args);
        if (!injected && args[1] === format) {
          injected = true;
          const data = await fsp.readFile(args[0]);
          let mutated;
          if (format === 'jpeg') {
            const eoi = data.lastIndexOf(Buffer.from([0xff, 0xd9]));
            const payload = Buffer.concat([Buffer.from('ICC_PROFILE\0', 'ascii'), Buffer.from([1, 1]), Buffer.from([0, 0, 0, 4])]);
            const segment = Buffer.concat([Buffer.from([0xff, 0xe2]), Buffer.alloc(2), payload]);
            segment.writeUInt16BE(segment.length - 2, 2);
            mutated = Buffer.concat([data.subarray(0, eoi), segment, data.subarray(eoi)]);
          } else if (format === 'webp') {
            const payload = Buffer.from([0, 1, 2, 3]);
            const chunk = Buffer.alloc(8 + payload.length + (payload.length % 2));
            chunk.write('ICCP', 0, 'ascii');
            chunk.writeUInt32LE(payload.length, 4);
            payload.copy(chunk, 8);
            const riff = Buffer.from(data);
            mutated = Buffer.concat([riff, chunk]);
            mutated.writeUInt32LE(mutated.length - 8, 4);
          } else {
            const avif = Buffer.from(data);
            const colr = avif.indexOf(Buffer.from('colr', 'ascii'));
            assert.ok(colr > 4);
            avif.write('prof', colr + 4, 'ascii');
            avif.fill(0, colr + 12, colr + 19);
            mutated = avif;
          }
          await fsp.writeFile(args[0], mutated);
          return {
            ...result,
            bytesWritten: mutated.length,
            metrics: { ...result.metrics, bytesOut: mutated.length },
          };
        }
        return result;
      };
      try {
        await assertRejected(
          () => compileImage({
            inputPath: INPUT,
            outputDir: path.join(parent, `${format}-malformed-icc-output`),
            policy: { widths: [320], formats: [format], placeholder: false },
          }),
          (error) => {
            assert.equal(error.name, 'ArtifactCompilationError');
            assert.equal(error.phase, 'verification');
            assert.equal(error.errorCode, 'E900');
          },
        );
      } finally {
        ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
      }
      assert.equal(await fsp.stat(path.join(parent, `${format}-malformed-icc-output`)).catch(() => null), null);
    });
  }

  for (const mutation of [
    { format: 'jpeg', name: 'jfif-payload', needle: Buffer.from('JFIF\0'), mutate(data, offset) {
      const marker = data.indexOf(Buffer.from([0xff, 0xe0]), 2);
      assert.ok(marker > 0);
      const length = data.readUInt16BE(marker + 2);
      const payload = Buffer.concat([data.subarray(marker + 4, marker + 2 + length), Buffer.from('PRIVATE')]);
      const segment = Buffer.alloc(4 + payload.length);
      segment[0] = 0xff;
      segment[1] = 0xe0;
      segment.writeUInt16BE(payload.length + 2, 2);
      payload.copy(segment, 4);
      return Buffer.concat([data.subarray(0, marker), segment, data.subarray(marker + 2 + length)]);
    } },
    { format: 'jpeg', name: 'jfxx-segment', needle: Buffer.from('JFIF\0'), mutate(data, offset) {
      const mutated = Buffer.from(data);
      mutated.write('JFXX', offset, 'ascii');
      return mutated;
    } },
    { format: 'avif', name: 'ftyp-minor', needle: Buffer.from('ftyp'), mutate(data, offset) {
      const mutated = Buffer.from(data);
      mutated[offset + 8] = 1;
      return mutated;
    } },
    { format: 'avif', name: 'ftyp-brand', needle: Buffer.from('ftyp'), mutate(data, offset) {
      const mutated = Buffer.from(data);
      mutated.write('evil', offset + 12, 'ascii');
      return mutated;
    } },
    { format: 'avif', name: 'hdlr-payload', needle: Buffer.from('libavif\0'), mutate(data, offset) {
      const mutated = Buffer.from(data);
      mutated[offset] = 'x'.charCodeAt(0);
      return mutated;
    } },
    { format: 'avif', name: 'infe-payload', needle: Buffer.from('Color\0'), mutate(data, offset) {
      const mutated = Buffer.from(data);
      mutated[offset] = 'S'.charCodeAt(0);
      return mutated;
    } },
  ]) {
    await withTempParent(async (parent) => {
      const originalToFileWithMetrics = ImageEngine.prototype.toFileWithMetrics;
      let injected = false;
      ImageEngine.prototype.toFileWithMetrics = async function patchedPrivatePayload(...args) {
        const result = await originalToFileWithMetrics.apply(this, args);
        if (!injected && args[1] === mutation.format) {
          injected = true;
          const data = await fsp.readFile(args[0]);
          const offset = data.indexOf(mutation.needle);
          assert.ok(offset >= 0, `${mutation.name} fixture should contain its canonical payload`);
          const mutated = mutation.mutate(data, offset);
          await fsp.writeFile(args[0], mutated);
          return {
            ...result,
            bytesWritten: mutated.length,
            metrics: { ...result.metrics, bytesOut: mutated.length },
          };
        }
        return result;
      };
      try {
        await assertRejected(
          () => compileImage({
            inputPath: INPUT,
            outputDir: path.join(parent, `${mutation.name}-output`),
            policy: { widths: [320], formats: [mutation.format], placeholder: false },
          }),
          (error) => {
            assert.equal(error.name, 'ArtifactCompilationError');
            assert.equal(error.phase, 'verification');
            assert.equal(error.errorCode, 'E900');
          },
        );
      } finally {
        ImageEngine.prototype.toFileWithMetrics = originalToFileWithMetrics;
      }
      assert.equal(await fsp.stat(path.join(parent, `${mutation.name}-output`)).catch(() => null), null);
    });
  }
}

main().then(
  () => console.log('artifact compiler contract passed'),
  (error) => {
    console.error(error);
    process.exitCode = 1;
  },
);
