'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths')
const { ImageEngine, inspectFile } = require(resolveRoot('index'))

const INPUT = resolveFixture('test_100KB_1057x1057.jpg')
const OPTIONS = { targetBytes: 20_000, minQuality: 40, maxQuality: 90 }

async function main() {
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
