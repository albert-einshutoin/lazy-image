/**
 * Canonical lazy-image vs sharp comparison.
 *
 * Quality is gated against a resized lossless source reference. Similarity
 * between the two encoders is reported separately and is never treated as
 * proof that either output retained source quality.
 */

'use strict'

const fs = require('node:fs')
const path = require('node:path')
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths')
const { ImageEngine } = require(resolveRoot('index'))
const { calculateQualityMetrics } = require('../helpers/quality')

const TEST_IMAGE = resolveFixture('test_4.5MB_5000x5000.png')
const DEFAULT_QUALITY_ARTIFACT = resolveTemp(
  'benchmarks',
  'sharp-comparison-quality.json',
)

const TEST_CASES = [
  { format: 'jpeg', quality: 80, name: 'JPEG', required: true },
  { format: 'webp', quality: 80, name: 'WebP', required: true },
  { format: 'avif', quality: 60, name: 'AVIF', required: true },
]

// These are source-reference regression floors, calibrated below the current
// canonical snapshot to tolerate normal cross-platform metric noise while
// still rejecting a material codec-quality regression.
const SOURCE_QUALITY_TARGETS = {
  jpeg: { ssim: 0.9935, psnr: 36.5 },
  webp: { ssim: 0.9905, psnr: 36.5 },
  avif: { ssim: 0.9940, psnr: 38.5 },
}

function calcDiff(lazy, sharp) {
  return ((lazy - sharp) / sharp) * 100
}

function isUnsupported(error) {
  const message = String(error?.message || error).toLowerCase()
  return /unsupported|not support|not available|no codec/.test(message)
}

function qualityPasses(metrics, target) {
  return metrics.ssim >= target.ssim && metrics.psnr >= target.psnr
}

function requiredFailures(results) {
  return results.filter(
    (result) =>
      result.required &&
      ['unsupported', 'benchmark-failure', 'quality-regression', 'skipped'].includes(
        result.status,
      ),
  )
}

function assertRequiredCasesPassed(results) {
  const failures = requiredFailures(results)
  if (failures.length === 0) return
  const summary = failures
    .map((result) => `${result.name}: ${result.status} (${result.reason})`)
    .join('; ')
  throw new Error(`Benchmark gate failed after all cases completed: ${summary}`)
}

async function benchmarkLazy(format, quality) {
  const start = Date.now()
  const buffer = await ImageEngine.fromPath(TEST_IMAGE)
    .resize(800, null)
    .toBuffer(format, quality)
  return { size: buffer.length, time: Date.now() - start, buffer }
}

async function createSharpAdapter() {
  let sharp
  try {
    sharp = require('sharp')
  } catch (error) {
    throw new Error(`sharp is not installed: ${error.message}`)
  }

  return {
    async encode(format, quality) {
      const start = Date.now()
      let pipeline = sharp(TEST_IMAGE).resize(800, null, { withoutEnlargement: true })
      if (format === 'jpeg') pipeline = pipeline.jpeg({ quality, mozjpeg: true })
      else if (format === 'webp') pipeline = pipeline.webp({ quality })
      else if (format === 'avif') pipeline = pipeline.avif({ quality })
      else throw new Error(`Unsupported format: ${format}`)
      const buffer = await pipeline.toBuffer()
      return { size: buffer.length, time: Date.now() - start, buffer }
    },
    reference() {
      return sharp(TEST_IMAGE)
        .resize(800, null, { withoutEnlargement: true })
        .png({ compressionLevel: 0 })
        .toBuffer()
    },
    async complex() {
      const start = Date.now()
      const buffer = await sharp(TEST_IMAGE)
        .resize(800, null, { withoutEnlargement: true })
        .rotate(90)
        .greyscale()
        .jpeg({ quality: 75, mozjpeg: true })
        .toBuffer()
      return { size: buffer.length, time: Date.now() - start }
    },
  }
}

function writeJson(outputPath, value) {
  fs.mkdirSync(path.dirname(outputPath), { recursive: true })
  fs.writeFileSync(outputPath, `${JSON.stringify(value, null, 2)}\n`)
}

async function runBenchmark(options = {}) {
  const log = options.log || console
  const cases = options.cases || TEST_CASES
  const targets = options.targets || SOURCE_QUALITY_TARGETS
  const lazyEncode = options.lazyEncode || benchmarkLazy
  const sharpAdapter = options.sharpAdapter || (await createSharpAdapter())
  const metrics = options.calculateMetrics || calculateQualityMetrics
  const reference = await sharpAdapter.reference()
  const results = []
  const summaryMetrics = []
  const performanceOnly = []

  log.log('=== lazy-image vs sharp Benchmark ===')
  log.log(`Input: ${TEST_IMAGE}`)
  log.log('Canonical quality axis: encoder output vs resized lossless source reference')

  for (const testCase of cases) {
    const result = {
      name: testCase.name,
      format: testCase.format,
      quality: testCase.quality,
      required: testCase.required !== false,
      status: 'benchmark-failure',
      reason: 'case did not complete',
    }
    results.push(result)

    if (testCase.skip) {
      result.status = 'skipped'
      result.reason = testCase.skip
      continue
    }

    try {
      let lazy
      try {
        lazy = await lazyEncode(testCase.format, testCase.quality)
      } catch (error) {
        if (isUnsupported(error)) {
          result.status = 'unsupported'
          result.reason = `lazy-image: ${error.message}`
          continue
        }
        throw error
      }
      let sharpResult
      try {
        sharpResult = await sharpAdapter.encode(testCase.format, testCase.quality)
      } catch (error) {
        if (isUnsupported(error)) {
          result.status = 'unsupported'
          result.reason = error.message
          result.lazy = { size: lazy.size, time: lazy.time }
          continue
        }
        throw error
      }

      const [lazyVsSharp, lazyVsReference, sharpVsReference] = await Promise.all([
        metrics(lazy.buffer, sharpResult.buffer),
        metrics(lazy.buffer, reference),
        metrics(sharpResult.buffer, reference),
      ])
      const target = targets[testCase.format]
      const passed = target && qualityPasses(lazyVsReference, target)

      Object.assign(result, {
        status: passed ? 'pass' : 'quality-regression',
        reason: passed
          ? 'source-reference quality target met'
          : `source-reference target not met (SSIM >= ${target?.ssim}, PSNR >= ${target?.psnr})`,
        target,
        lazy: { size: lazy.size, time: lazy.time },
        sharp: { size: sharpResult.size, time: sharpResult.time },
        axes: { lazyVsSharp, lazyVsReference, sharpVsReference },
        sizeDifferencePercent: calcDiff(lazy.size, sharpResult.size),
      })

      summaryMetrics.push(
        { name: `${testCase.name} size (lazy-image)`, value: lazy.size, unit: 'bytes' },
        { name: `${testCase.name} time (lazy-image)`, value: lazy.time, unit: 'ms' },
      )
    } catch (error) {
      result.status = 'benchmark-failure'
      result.reason = error.message
    }
  }

  // Complex pipeline is reported for performance only; it has no canonical
  // quality target and therefore cannot mask or replace format gate failures.
  if (options.includeComplex !== false) {
    try {
      const start = Date.now()
      const lazy = await ImageEngine.fromPath(TEST_IMAGE)
        .resize(800, null)
        .rotate(90)
        .grayscale()
        .toBuffer('jpeg', 75)
      const lazyTime = Date.now() - start
      const sharpResult = await sharpAdapter.complex()
      performanceOnly.push({
        name: 'Complex Pipeline',
        lazy: { size: lazy.length, time: lazyTime },
        sharp: sharpResult,
      })
      summaryMetrics.push(
        { name: 'Complex pipeline size (lazy-image)', value: lazy.length, unit: 'bytes' },
        { name: 'Complex pipeline time (lazy-image)', value: lazyTime, unit: 'ms' },
      )
      log.log(`Complex pipeline: lazy ${lazy.length} bytes, sharp ${sharpResult.size} bytes`)
    } catch (error) {
      results.push({
        name: 'Complex Pipeline',
        required: false,
        status: 'benchmark-failure',
        reason: error.message,
      })
    }
  }

  for (const result of results) {
    log.log(`${result.name}: ${result.status} — ${result.reason}`)
  }

  const metricsPath = options.metricsOutputPath ?? process.env.BENCHMARK_OUTPUT_JSON
  if (metricsPath) writeJson(metricsPath, summaryMetrics)
  const qualityPath =
    options.qualityOutputPath ??
    process.env.BENCHMARK_QUALITY_OUTPUT_JSON ??
    DEFAULT_QUALITY_ARTIFACT
  writeJson(qualityPath, {
    schemaVersion: 1,
    input: TEST_IMAGE,
    resizeWidth: 800,
    results,
    performanceOnly,
  })
  log.log(`Quality artifact: ${qualityPath}`)

  assertRequiredCasesPassed(results)
  return { results, summaryMetrics, qualityPath }
}

if (require.main === module) {
  runBenchmark().catch((error) => {
    console.error(error.message)
    process.exitCode = 1
  })
}

module.exports = {
  SOURCE_QUALITY_TARGETS,
  assertRequiredCasesPassed,
  qualityPasses,
  requiredFailures,
  runBenchmark,
}
