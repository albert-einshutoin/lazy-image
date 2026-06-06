const { resolveFixture } = require('./paths');

const FUZZ_TARGET_SEED_COVERAGE = {
  decode_avif: [
    'avif-ftyp-only.bin',
    'avif-invalid-box-size.bin',
    'avif-truncated-meta-box.bin',
  ],
  decode_from_buffer: [
    'jpeg-invalid-segment-length.bin',
    'jpeg-soi-only.bin',
    'png-huge-ihdr.bin',
    'webp-truncated-riff.bin',
  ],
  exif_parse: [
    'jpeg-truncated-exif-app1.bin',
  ],
  icc_profile: [
    'jpeg-truncated-icc-app2.bin',
  ],
  inspect_header: [
    'jpeg-invalid-segment-length.bin',
    'jpeg-soi-only.bin',
    'png-huge-ihdr.bin',
    'webp-truncated-riff.bin',
  ],
};

const JPEG_BACKEND_FUZZ_TARGETS = [
  'decode_from_buffer',
  'exif_parse',
  'icc_profile',
  'inspect_header',
];

const AVIF_BACKEND_FUZZ_TARGETS = [
  'decode_avif',
  'decode_from_buffer',
  'icc_profile',
  'inspect_header',
];

const JPEG_BACKEND_BAKEOFF_CASES = [
  {
    label: 'large-png-default-jpeg',
    input: resolveFixture('test_4.5MB_5000x5000.png'),
    width: 800,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'photo-jpeg-default-jpeg',
    input: resolveFixture('test_3.2MB_5000x5000.jpg'),
    width: 800,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'small-jpeg-default-jpeg',
    input: resolveFixture('test_38kb_input.jpg'),
    width: 800,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'mid-png-smaller-output',
    input: resolveFixture('test_100KB_1188x1188.png'),
    width: 400,
    mozjpegQuality: 75,
    jpegliDistance: 1.4,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'generated-gradient-default-jpeg',
    source: 'generated-smooth-gradient',
    width: 512,
    height: 320,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'generated-ui-default-jpeg',
    source: 'generated-ui-screenshot',
    width: 640,
    height: 360,
    mozjpegQuality: 80,
    jpegliDistance: 1.0,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'generated-texture-high-detail',
    source: 'generated-high-detail-texture',
    width: 512,
    height: 512,
    mozjpegQuality: 85,
    jpegliDistance: 0.8,
    fuzzTargets: JPEG_BACKEND_FUZZ_TARGETS,
  },
];

const AVIF_BACKEND_BAKEOFF_CASES = [
  {
    label: 'large-png-q60',
    source: 'fixture',
    input: resolveFixture('test_4.5MB_5000x5000.png'),
    width: 800,
    quality: 60,
    fuzzTargets: AVIF_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'photo-jpeg-q75',
    source: 'fixture',
    input: resolveFixture('test_3.2MB_5000x5000.jpg'),
    width: 800,
    quality: 75,
    fuzzTargets: AVIF_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'alpha-gradient-q60',
    source: 'generated-alpha-gradient',
    width: 384,
    height: 256,
    quality: 60,
    expectAlpha: true,
    fuzzTargets: AVIF_BACKEND_FUZZ_TARGETS,
  },
  {
    label: 'icc-gradient-q60',
    source: 'generated-icc-gradient',
    width: 384,
    height: 256,
    quality: 60,
    expectIcc: true,
    fuzzTargets: AVIF_BACKEND_FUZZ_TARGETS,
  },
];

function formatFuzzTargets(targets) {
  return [...new Set(targets)].sort().join('+');
}

function buildBakeoffCorpusExpectations(testCase) {
  const expectations = {
    fuzzTargets: formatFuzzTargets(testCase.fuzzTargets || []),
  };

  if (Object.hasOwn(testCase, 'expectAlpha')) {
    expectations.alpha = Boolean(testCase.expectAlpha);
  }

  if (Object.hasOwn(testCase, 'expectIcc')) {
    expectations.icc = Boolean(testCase.expectIcc);
  }

  return expectations;
}

module.exports = {
  AVIF_BACKEND_BAKEOFF_CASES,
  FUZZ_TARGET_SEED_COVERAGE,
  JPEG_BACKEND_BAKEOFF_CASES,
  buildBakeoffCorpusExpectations,
};
