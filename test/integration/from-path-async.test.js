'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const { resolveFixture, resolveRoot, resolveTemp } = require('../helpers/paths')
const { ImageEngine, inspect } = require(resolveRoot('index'))

const INPUT = resolveFixture('test_100KB_1057x1057.jpg')

async function main() {
  const engine = await ImageEngine.fromPathAsync(INPUT)
  const output = await engine.resize(120).toBuffer('jpeg', 80)
  assert.equal(inspect(output).width, 120)

  const missing = resolveTemp('from-path-async-missing.jpg')
  let syncError
  let asyncError
  try {
    ImageEngine.fromPath(missing)
  } catch (error) {
    syncError = error
  }
  try {
    await ImageEngine.fromPathAsync(missing)
  } catch (error) {
    asyncError = error
  }
  assert(syncError, 'sync path should reject a missing file')
  assert(asyncError, 'async path should reject a missing file')
  assert.equal(asyncError.code, syncError.code)
  assert.equal(asyncError.category, syncError.category)

  const largePath = resolveTemp('from-path-async-128mb.bin')
  const fd = fs.openSync(largePath, 'w')
  fs.ftruncateSync(fd, 128 * 1024 * 1024)
  fs.closeSync(fd)
  try {
    let loadFinished = false
    const load = ImageEngine.fromPathAsync(largePath).then((loaded) => {
      loadFinished = true
      return loaded
    })
    let timerObservedInFlightRead = false
    await new Promise((resolve) =>
      setTimeout(() => {
        timerObservedInFlightRead = !loadFinished
        resolve()
      }, 0),
    )
    await load
    assert(
      timerObservedInFlightRead,
      'timer should run while the Rust worker is still reading the file source',
    )
  } finally {
    fs.rmSync(largePath, { force: true })
  }

  console.log('fromPathAsync tests passed')
}

main().catch((error) => {
  console.error(error)
  process.exitCode = 1
})
