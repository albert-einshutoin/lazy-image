'use strict'

const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
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

  const originalCwd = process.cwd()
  const relativeRoot = fs.mkdtempSync(resolveTemp('from-path-relative-'))
  const sourceCwd = path.join(relativeRoot, 'source')
  const otherCwd = path.join(relativeRoot, 'other')
  fs.mkdirSync(sourceCwd)
  fs.mkdirSync(otherCwd)
  fs.copyFileSync(INPUT, path.join(sourceCwd, 'input.jpg'))
  try {
    process.chdir(sourceCwd)
    const relativeLoad = ImageEngine.fromPathAsync('input.jpg')
    process.chdir(otherCwd)
    const relativeEngine = await relativeLoad
    const relativeOutput = await relativeEngine.resize(64).toBuffer('jpeg', 80)
    assert.equal(inspect(relativeOutput).width, 64)
  } finally {
    process.chdir(originalCwd)
    fs.rmSync(relativeRoot, { recursive: true, force: true })
  }

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
