'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const sharp = require('sharp')
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths')
const { ImageEngine, inspectFile } = require(resolveRoot('index'))

const INPUT = resolveFixture('test_100KB_1057x1057.jpg')
const OPTIONS = { targetBytes: 20_000, minQuality: 40, maxQuality: 90 }

async function main() {
  const solid = await sharp({ create: {
    width: 64, height: 64, channels: 3, background: { r: 200, g: 100, b: 50 },
  } }).png().toBuffer()
  const solidPath = resolveTemp('target-bytes-nonmonotonic.jpg')
  try {
    for (const fastMode of [false, true]) {
      const maximum = await ImageEngine.from(solid).toBuffer('jpeg', 100, fastMode)
      if (!fastMode) {
        const midpoint = await ImageEngine.from(solid).toBuffer('jpeg', 50, false)
        assert.ok(midpoint.length > maximum.length, 'normal JPEG fixture is non-monotonic')
      }
      const options = { targetBytes: maximum.length, minQuality: 1, maxQuality: 100,
        fastMode, qualityFloorPolicy: 'strict' }
      const buffer = await ImageEngine.from(solid).toBufferTargetBytes('jpeg', options)
      assert.equal(buffer.quality, 100, 'a feasible maximum must not be discarded')
      assert.equal(buffer.budgetMet, true)
      assert.deepEqual(buffer.data, maximum)
      const file = await ImageEngine.from(solid).toFileTargetBytes(solidPath, 'jpeg', options)
      assert.equal(file.quality, 100)
      assert.equal(file.budgetMet, true)
      assert.deepEqual(fs.readFileSync(solidPath), maximum)
    }
  } finally {
    fs.rmSync(solidPath, { force: true })
  }

  const outputPath = resolveTemp('target-bytes-native.jpg')
  try {
    fs.writeFileSync(outputPath, 'existing destination')
    const [bufferResult, fileResult] = await Promise.all([
      ImageEngine.fromPath(INPUT).resize(400).toBufferTargetBytes('jpeg', OPTIONS),
      ImageEngine.fromPath(INPUT)
        .resize(400)
        .toFileTargetBytes(outputPath, 'jpeg', OPTIONS),
    ])

    assert.equal(fileResult.quality, bufferResult.quality)
    assert.equal(fileResult.bytesWritten, bufferResult.bytesOut)
    assert.equal(fileResult.budgetMet, bufferResult.budgetMet)
    assert.equal(fileResult.targetBytes, bufferResult.targetBytes)
    assert.equal('data' in fileResult, false, 'file result must not expose a V8 Buffer')
    assert.equal(fs.statSync(outputPath).size, fileResult.bytesWritten)
    assert.equal(inspectFile(outputPath).format, 'jpeg')
  } finally {
    fs.rmSync(outputPath, { force: true })
  }

  const originalCwd = process.cwd()
  const bareNameDirectory = fs.mkdtempSync(resolveTemp('target-bytes-bare-'))
  try {
    process.chdir(bareNameDirectory)
    const bareNameResult = await ImageEngine.fromPath(INPUT)
      .resize(400)
      .toFileTargetBytes('output.jpg', 'jpeg', OPTIONS)
    assert.equal(fs.statSync('output.jpg').size, bareNameResult.bytesWritten)
  } finally {
    process.chdir(originalCwd)
    fs.rmSync(bareNameDirectory, { recursive: true, force: true })
  }

  const protectedPath = resolveTemp('target-bytes-native-protected.jpg')
  fs.writeFileSync(protectedPath, 'keep this file')
  try {
    await assert.rejects(
      ImageEngine.fromPath(INPUT).toFileTargetBytes(protectedPath, 'jpeg', {
        targetBytes: 1,
        minQuality: 90,
        maxQuality: 90,
        qualityFloorPolicy: 'strict',
      }),
      /E300|Unable to meet targetBytes/,
    )
    assert.equal(
      fs.readFileSync(protectedPath, 'utf8'),
      'keep this file',
      'strict search failure must leave the destination untouched',
    )
  } finally {
    fs.rmSync(protectedPath, { force: true })
  }

  const directoryAsOutput = resolveTemp('target-bytes-output-directory')
  fs.mkdirSync(directoryAsOutput, { recursive: true })
  try {
    await assert.rejects(
      ImageEngine.fromPath(INPUT).toFileTargetBytes(directoryAsOutput, 'jpeg', OPTIONS),
      /E301|write/i,
    )
  } finally {
    fs.rmSync(directoryAsOutput, { recursive: true, force: true })
  }

  console.log('native target-bytes file tests passed')
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
