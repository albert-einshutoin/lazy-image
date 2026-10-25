'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const { resolveTemp } = require('../helpers/paths')
const {
  assertRequiredCasesPassed,
  runBenchmark,
} = require('../benchmarks/sharp-comparison.bench')
const { analyzePairs } = require('../../scripts/bench/paired-comparison.cjs')

const outputPath = resolveTemp('benchmark-quality-gate-test.json')
const attempted = []
const observed = []
const cases = [
  { name: 'Pass', format: 'pass', quality: 80, required: true },
  { name: 'Regression', format: 'regression', quality: 80, required: true },
  { name: 'Unsupported', format: 'unsupported', quality: 80, required: true },
  { name: 'Failure', format: 'failure', quality: 80, required: true },
  { name: 'Skipped', format: 'skipped', quality: 80, required: false, skip: 'optional' },
]

const sharpAdapter = {
  async reference() {
    return Buffer.from('reference')
  },
  async encode(format) {
    attempted.push(`sharp:${format}`)
    if (format === 'unsupported') throw new Error('unsupported codec: unsupported')
    if (format === 'failure') throw new Error('encoder crashed')
    return { size: 20, time: 2, buffer: Buffer.from(`sharp:${format}`) }
  },
}

async function main() {
  await assert.rejects(
    runBenchmark({
      cases,
      targets: {
        pass: { ssim: 0.9, psnr: 30 },
        regression: { ssim: 0.9, psnr: 30 },
        unsupported: { ssim: 0.9, psnr: 30 },
        failure: { ssim: 0.9, psnr: 30 },
      },
      lazyEncode: async (format) => {
        attempted.push(`lazy:${format}`)
        return { size: 10, time: 1, buffer: Buffer.from(`lazy:${format}`) }
      },
      sharpAdapter,
      calculateMetrics: async (left) =>
        left.toString() === 'lazy:regression'
          ? { ssim: 0.5, psnr: 10 }
          : { ssim: 0.99, psnr: 40 },
      onOutput: async ({ name, lazy, sharp }) => {
        observed.push({ name, lazy: lazy.toString(), sharp: sharp.toString() })
      },
      qualityOutputPath: outputPath,
      metricsOutputPath: null,
      includeComplex: false,
      log: { log() {} },
    }),
    /Benchmark gate failed after all cases completed/,
  )

  assert.deepEqual(attempted, [
    'lazy:pass',
    'sharp:pass',
    'lazy:regression',
    'sharp:regression',
    'lazy:unsupported',
    'sharp:unsupported',
    'lazy:failure',
    'sharp:failure',
  ])

  const artifact = JSON.parse(fs.readFileSync(outputPath, 'utf8'))
  assert.deepEqual(
    artifact.results.map(({ status }) => status),
    ['pass', 'quality-regression', 'unsupported', 'benchmark-failure', 'skipped'],
  )
  assert(artifact.results[0].axes.lazyVsReference)
  assert.deepEqual(observed, [
    { name: 'Pass', lazy: 'lazy:pass', sharp: 'sharp:pass' },
    { name: 'Regression', lazy: 'lazy:regression', sharp: 'sharp:regression' },
  ])
  assert.throws(
    () => assertRequiredCasesPassed(artifact.results),
    /Regression: quality-regression/,
  )
  fs.rmSync(outputPath, { force: true })
  const sample = (value, webp = value) => ({ samples: ['first', 'warm1', 'warm2', 'warm3'].map(phase => ({
    phase,
    timings: Object.fromEntries(['JPEG', 'WebP', 'AVIF', 'Complex Pipeline'].map(name => [name, {
      lazy: name === 'WebP' ? webp : value, sharp: value,
    }])),
  })) })
  const pairs = values => values.map(value => ({ a: sample(100), b: sample(value) }))
  assert.equal(analyzePairs(pairs([110, 110, 110, 110, 110])).verdict, 'PASS')
  assert.equal(analyzePairs(pairs([116, 116, 116, 116, 116])).verdict, 'REGRESSION')
  assert.equal(analyzePairs(pairs([100, 100, 120, 120, 120])).verdict, 'INCONCLUSIVE')
  assert.equal(analyzePairs(Array.from({ length: 5 }, () => ({ a: sample(100), b: sample(100, 200) }))).verdict, 'PASS')
  assert.throws(() => analyzePairs(pairs([0, 100, 100, 100, 100])), /invalid timing/)
  console.log('benchmark quality gate test passed')
}

main().catch((error) => {
  fs.rmSync(outputPath, { force: true })
  console.error(error)
  process.exitCode = 1
})
