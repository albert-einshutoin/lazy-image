#!/usr/bin/env node

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..', '..');

function parseArgs(argv) {
  const options = {
    corpusRoot: path.join(repoRoot, 'fuzz', 'corpus'),
    dryRun: false,
    target: undefined,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === '--target') {
      options.target = argv[i + 1];
      i += 1;
    } else if (arg === '--corpus-root') {
      options.corpusRoot = path.resolve(argv[i + 1]);
      i += 1;
    } else if (arg === '--dry-run') {
      options.dryRun = true;
    } else {
      throw new Error(`Unknown option: ${arg}`);
    }
  }

  return options;
}

function pngHugeDimensions() {
  const ihdr = Buffer.alloc(13);
  ihdr.writeUInt32BE(0x7fffffff, 0);
  ihdr.writeUInt32BE(0x7fffffff, 4);
  ihdr[8] = 8; // bit depth
  ihdr[9] = 2; // RGB

  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    Buffer.from([0x00, 0x00, 0x00, 0x0d]),
    Buffer.from('IHDR', 'ascii'),
    ihdr,
    Buffer.alloc(4), // intentionally invalid CRC
  ]);
}

function jpegWithTruncatedAppSegment(marker) {
  return Buffer.from([
    0xff, 0xd8,
    0xff, marker,
    0x00, 0x20,
    0x4c, 0x41, 0x5a, 0x59,
  ]);
}

function jpegWithInvalidSegmentLength() {
  return Buffer.from([
    0xff, 0xd8,
    0xff, 0xe0,
    0x00, 0x01,
    0xff, 0xd9,
  ]);
}

function truncatedRiffWebp() {
  return Buffer.from([
    0x52, 0x49, 0x46, 0x46,
    0xff, 0xff, 0xff, 0x7f,
    0x57, 0x45, 0x42, 0x50,
    0x56, 0x50, 0x38, 0x20,
  ]);
}

function avifFtypOnly() {
  return Buffer.from([
    0x00, 0x00, 0x00, 0x18,
    0x66, 0x74, 0x79, 0x70,
    0x61, 0x76, 0x69, 0x66,
    0x00, 0x00, 0x00, 0x00,
    0x61, 0x76, 0x69, 0x66,
    0x6d, 0x69, 0x66, 0x31,
  ]);
}

function avifTruncatedMetaBox() {
  return Buffer.concat([
    avifFtypOnly(),
    Buffer.from([
      0x00, 0x00, 0x00, 0x20,
      0x6d, 0x65, 0x74, 0x61,
      0x00, 0x00, 0x00, 0x00,
    ]),
  ]);
}

const headerMalformedSeeds = {
  'empty.bin': Buffer.alloc(0),
  'random-short.bin': Buffer.from([0x42, 0x00, 0xff, 0x13, 0x7f, 0x80, 0x01, 0x02]),
  'jpeg-soi-only.bin': Buffer.from([0xff, 0xd8, 0xff, 0xe0, 0x00, 0x10]),
  'jpeg-invalid-segment-length.bin': jpegWithInvalidSegmentLength(),
  'png-huge-ihdr.bin': pngHugeDimensions(),
  'webp-truncated-riff.bin': truncatedRiffWebp(),
};

const seedsByTarget = {
  decode_from_buffer: headerMalformedSeeds,
  inspect_header: headerMalformedSeeds,
  icc_profile: {
    'jpeg-truncated-icc-app2.bin': jpegWithTruncatedAppSegment(0xe2),
    'jpeg-invalid-segment-length.bin': jpegWithInvalidSegmentLength(),
  },
  exif_parse: {
    'jpeg-truncated-exif-app1.bin': jpegWithTruncatedAppSegment(0xe1),
    'little-endian-tiff-header-only.bin': Buffer.from('II*\0', 'binary'),
    'big-endian-tiff-header-only.bin': Buffer.from('MM\0*', 'binary'),
  },
  decode_avif: {
    'avif-ftyp-only.bin': avifFtypOnly(),
    'avif-truncated-meta-box.bin': avifTruncatedMetaBox(),
    'avif-invalid-box-size.bin': Buffer.from([
      0x00, 0x00, 0x00, 0x00,
      0x66, 0x74, 0x79, 0x70,
      0x61, 0x76, 0x69, 0x66,
    ]),
  },
};

function selectedTargets(target) {
  if (target) {
    return Object.hasOwn(seedsByTarget, target) ? [target] : [];
  }
  return Object.keys(seedsByTarget).sort();
}

function writeSeed(targetDir, name, data, dryRun) {
  const outputPath = path.join(targetDir, name);
  if (!dryRun) {
    fs.mkdirSync(targetDir, { recursive: true });
    fs.writeFileSync(outputPath, data);
  }
  return { outputPath, bytes: data.byteLength };
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const targets = selectedTargets(options.target);
  let written = 0;

  for (const target of targets) {
    const targetDir = path.join(options.corpusRoot, target);
    for (const [name, data] of Object.entries(seedsByTarget[target])) {
      const result = writeSeed(targetDir, name, data, options.dryRun);
      written += 1;
      console.log(`${options.dryRun ? 'would write' : 'wrote'} ${result.outputPath} (${result.bytes} bytes)`);
    }
  }

  if (options.target && targets.length === 0) {
    console.log(`no malformed seed corpus configured for target: ${options.target}`);
  } else {
    console.log(`${options.dryRun ? 'validated' : 'wrote'} ${written} malformed fuzz seed(s)`);
  }
}

main();
