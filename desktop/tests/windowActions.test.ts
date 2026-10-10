import { test } from 'node:test'
import assert from 'node:assert/strict'
import { windowAction } from '../src/windowActions.ts'

function window(fullscreen = false) {
  const calls: string[] = []
  return { calls, minimize: async () => { calls.push('minimize') }, close: async () => { calls.push('close') },
    isFullscreen: async () => fullscreen, setFullscreen: async (value: boolean) => { calls.push(`fullscreen:${value}`) },
    toggleMaximize: async () => { calls.push('toggle-maximize') } }
}

test('custom close uses the normal close request so native close-to-tray settings are respected', async () => {
  const target = window()
  await windowAction(target, 'close')
  assert.deepEqual(target.calls, ['close'])
})
test('maximize toggles the window, exits OS fullscreen and is disabled for a mini player', async () => {
  const target = window(), full = window(true), mini = window()
  await windowAction(target, 'maximize')
  await windowAction(full, 'maximize')
  await windowAction(mini, 'maximize', true)
  assert.deepEqual(target.calls, ['toggle-maximize'])
  assert.deepEqual(full.calls, ['fullscreen:false'])
  assert.deepEqual(mini.calls, [])
})
