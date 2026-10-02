import assert from 'node:assert/strict'
import { test } from 'node:test'
import { displayReleaseVersion, releaseIsNewer, startUpdatePolling } from '../src/updatePolling.ts'

function environment() {
  const targetWindow = Object.assign(new EventTarget(), { setInterval, clearInterval })
  const targetDocument = Object.assign(new EventTarget(), { visibilityState: 'visible' })
  return { targetWindow, targetDocument }
}

test('a release signal checks immediately without waiting for the fallback timer', t => {
  t.mock.timers.enable({ apis: ['Date', 'setInterval'], now: 100_000 })
  const { targetWindow, targetDocument } = environment()
  let published = false
  let available = false
  const polling = startUpdatePolling(() => { available = published }, targetWindow, targetDocument)
  assert.equal(available, false)
  published = true
  t.mock.timers.tick(1)
  assert.equal(available, false)
  polling.published()
  assert.equal(available, true)
  polling.stop()
})

test('focus and tray resume check promptly without duplicate requests', t => {
  t.mock.timers.enable({ apis: ['Date', 'setInterval'], now: 100_000 })
  const { targetWindow, targetDocument } = environment()
  let checks = 0
  const polling = startUpdatePolling(() => { checks++ }, targetWindow, targetDocument)
  t.mock.timers.tick(599_999)
  targetWindow.dispatchEvent(new Event('focus'))
  assert.equal(checks, 1)
  t.mock.timers.tick(1)
  polling.resume() // Native Tauri focus event.
  targetWindow.dispatchEvent(new Event('focus'))
  assert.equal(checks, 2)
  polling.stop()
})

test('returning to a visible window and restoring the network trigger checks', t => {
  t.mock.timers.enable({ apis: ['Date', 'setInterval'], now: 100_000 })
  const { targetWindow, targetDocument } = environment()
  let checks = 0
  const polling = startUpdatePolling(() => { checks++ }, targetWindow, targetDocument)
  targetDocument.visibilityState = 'hidden'
  t.mock.timers.tick(600_000)
  targetDocument.dispatchEvent(new Event('visibilitychange'))
  assert.equal(checks, 1)
  targetDocument.visibilityState = 'visible'
  targetDocument.dispatchEvent(new Event('visibilitychange'))
  assert.equal(checks, 2)
  targetWindow.dispatchEvent(new Event('online'))
  assert.equal(checks, 3)
  polling.stop()
})

test('cleanup removes background checks and listeners before remount', t => {
  t.mock.timers.enable({ apis: ['Date', 'setInterval'], now: 100_000 })
  const { targetWindow, targetDocument } = environment()
  let checks = 0
  const first = startUpdatePolling(() => { checks++ }, targetWindow, targetDocument)
  first.stop()
  targetWindow.dispatchEvent(new Event('online'))
  const second = startUpdatePolling(() => { checks++ }, targetWindow, targetDocument)
  assert.equal(checks, 2)
  t.mock.timers.tick(30 * 60_000)
  assert.equal(checks, 3)
  second.stop()
  t.mock.timers.tick(30 * 60_000)
  assert.equal(checks, 3)
})

test('without a server signal the fallback checks only every thirty minutes', t => {
  t.mock.timers.enable({ apis: ['Date', 'setInterval'], now: 100_000 })
  const { targetWindow, targetDocument } = environment()
  let checks = 0
  const polling = startUpdatePolling(() => { checks++ }, targetWindow, targetDocument)
  t.mock.timers.tick(30 * 60_000 - 1)
  assert.equal(checks, 1)
  t.mock.timers.tick(1)
  assert.equal(checks, 2)
  polling.stop()
})

test('release signals compare version numbers rather than strings', () => {
  assert.equal(releaseIsNewer('0.2.10', '0.2.9'), true)
  assert.equal(releaseIsNewer('0.3.0', '0.2.99'), true)
  assert.equal(releaseIsNewer('0.2.1', '0.2.1'), false)
  assert.equal(releaseIsNewer('0.2.0', '0.2.1'), false)
  assert.equal(releaseIsNewer('bad', '0.2.1'), false)
  assert.equal(releaseIsNewer('0.2.1-a', '0.2.0'), true)
  assert.equal(releaseIsNewer('0.2.1-b', '0.2.1-a'), true)
  assert.equal(releaseIsNewer('0.2.1', '0.2.1-a'), true)
  assert.equal(releaseIsNewer('0.2.1-a', '0.2.1'), false)
  assert.equal(displayReleaseVersion('0.2.1-a'), '0.2.1a')
})
