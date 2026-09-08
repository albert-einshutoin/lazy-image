'use strict'

const fs = require('node:fs')
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths')
const { ImageEngine } = require(resolveRoot('index'))

const INPUT = resolveFixture('test_3.2MB_5000x5000.jpg')
const CONCURRENCY = 4
const OPTIONS = { targetBytes: 300_000, minQuality: 40, maxQuality: 90 }

function externalBytes() {
  const memory = process.memoryUsage()
  return memory.external + memory.arrayBuffers
}

async function main() {
  global.gc?.()
  const bufferBaseline = externalBytes()
  const bufferResults = await Promise.all(
    Array.from({ length: CONCURRENCY }, () =>
      ImageEngine.fromPath(INPUT).resize(1600).toBufferTargetBytes('jpeg', OPTIONS),
    ),
  )
  const bufferDelta = externalBytes() - bufferBaseline
  const bufferBytes = bufferResults.reduce((sum, result) => sum + result.data.length, 0)

  const outputPaths = Array.from({ length: CONCURRENCY }, (_, index) =>
    resolveTemp(`target-bytes-memory-${index}.jpg`),
  )
  try {
    global.gc?.()
    const fileBaseline = externalBytes()
    const fileResults = await Promise.all(
      outputPaths.map((outputPath) =>
        ImageEngine.fromPath(INPUT)
          .resize(1600)
          .toFileTargetBytes(outputPath, 'jpeg', OPTIONS),
      ),
    )
    const fileDelta = externalBytes() - fileBaseline

    if (fileResults.some((result) => 'data' in result)) {
      throw new Error('native file results must not expose encoded image buffers')
    }

    console.log('=== Target-bytes V8 output memory ===')
    console.log(`Concurrent outputs: ${CONCURRENCY}`)
    console.log(`Encoded buffer bytes retained in JS: ${bufferBytes}`)
    console.log(`Buffer variant external+arrayBuffers delta: ${bufferDelta}`)
    console.log(`File variant external+arrayBuffers delta: ${fileDelta}`)
    console.log('File result payload: metadata only (no data Buffer)')
  } finally {
    for (const outputPath of outputPaths) fs.rmSync(outputPath, { force: true })
  }
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
