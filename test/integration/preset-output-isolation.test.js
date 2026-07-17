'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const {
  resolveFixture,
  resolveRoot,
  resolveTemp,
} = require('../helpers/paths')
const { ImageEngine, inspect, inspectFile } = require(resolveRoot('index'))

const INPUT = resolveFixture('test_100KB_1057x1057.jpg')
const ORIGINAL_SIZE = { width: 1057, height: 1057 }

async function plainDimensions(engine) {
  return inspect(await engine.toBuffer('jpeg', 85))
}

function assertDimensions(actual, expected, message) {
  assert.deepEqual(
    { width: actual.width, height: actual.height },
    expected,
    message,
  )
}

async function run() {
  {
    const engine = ImageEngine.fromPath(INPUT)
    const preset = await engine.encode({ preset: 'thumbnail' })
    assertDimensions(inspect(preset.data), { width: 150, height: 150 })
    assertDimensions(
      await plainDimensions(engine),
      ORIGINAL_SIZE,
      'encode({ preset }) must not change later encode operations',
    )
  }

  {
    const engine = ImageEngine.fromPath(INPUT)
    const preset = await engine.encode({ preset: 'avatar', metrics: true })
    assertDimensions(inspect(preset.data), { width: 200, height: 200 })
    assert(preset.metrics, 'metrics should still be returned for preset output')
    assertDimensions(
      await plainDimensions(engine),
      ORIGINAL_SIZE,
      'metrics must not change preset isolation',
    )
  }

  {
    const engine = ImageEngine.fromPath(INPUT)
    const preset = await engine.toBufferWithPreset('thumbnail')
    assertDimensions(inspect(preset), { width: 150, height: 150 })
    assertDimensions(
      await plainDimensions(engine),
      ORIGINAL_SIZE,
      'toBufferWithPreset must not mutate queued operations',
    )
  }

  {
    const engine = ImageEngine.fromPath(INPUT)
    const preset = await engine.toBufferWithMetricsPreset('avatar')
    assertDimensions(inspect(preset.data), { width: 200, height: 200 })
    assertDimensions(
      await plainDimensions(engine),
      ORIGINAL_SIZE,
      'toBufferWithMetricsPreset must not mutate queued operations',
    )
  }

  {
    const engine = ImageEngine.fromPath(INPUT)
    const thumbnail = await engine.toBufferWithPreset('thumbnail')
    const avatar = await engine.toBufferWithPreset('avatar')
    assertDimensions(inspect(thumbnail), { width: 150, height: 150 })
    assertDimensions(
      inspect(avatar),
      { width: 200, height: 200 },
      'each preset must start from the caller pipeline, not a prior preset output',
    )
  }

  {
    const presetPath = resolveTemp('preset-isolation.webp')
    const plainPath = resolveTemp('preset-isolation-plain.jpg')
    const engine = ImageEngine.fromPath(INPUT)
    try {
      await engine.toFileWithPreset(presetPath, 'thumbnail')
      await engine.toFile(plainPath, 'jpeg', 85)
      assertDimensions(inspectFile(presetPath), { width: 150, height: 150 })
      assertDimensions(
        inspectFile(plainPath),
        ORIGINAL_SIZE,
        'toFileWithPreset must not mutate queued operations',
      )
    } finally {
      fs.rmSync(presetPath, { force: true })
      fs.rmSync(plainPath, { force: true })
    }
  }

  {
    const presetPath = resolveTemp('encode-preset-isolation.webp')
    const plainPath = resolveTemp('encode-preset-isolation-plain.jpg')
    const engine = ImageEngine.fromPath(INPUT)
    try {
      await engine.encodeToFile(presetPath, { preset: 'avatar', metrics: true })
      await engine.toFile(plainPath, 'jpeg', 85)
      assertDimensions(inspectFile(presetPath), { width: 200, height: 200 })
      assertDimensions(
        inspectFile(plainPath),
        ORIGINAL_SIZE,
        'encodeToFile({ preset }) must not mutate queued operations',
      )
    } finally {
      fs.rmSync(presetPath, { force: true })
      fs.rmSync(plainPath, { force: true })
    }
  }

  {
    const engine = ImageEngine.fromPath(INPUT)
    engine.preset('thumbnail')
    assertDimensions(
      await plainDimensions(engine),
      { width: 150, height: 150 },
      'deprecated preset() remains an explicitly mutating pipeline API',
    )
  }

  console.log('preset output isolation tests passed')
}

run().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
