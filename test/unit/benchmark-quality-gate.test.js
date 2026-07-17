'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const { resolveTemp } = require('../helpers/paths')
const {
  assertRequiredCasesPassed,
  runBenchmark,
} = require('../benchmarks/sharp-comparison.bench')

const outputPath = resolveTemp('benchmark-quality-gate-test.json')
const attempted = []
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
  assert.throws(
    () => assertRequiredCasesPassed(artifact.results),
    /Regression: quality-regression/,
  )
  fs.rmSync(outputPath, { force: true })
  console.log('benchmark quality gate test passed')
}

main().catch((error) => {
  fs.rmSync(outputPath, { force: true })
  console.error(error)
  process.exitCode = 1
})
