import { test } from 'node:test'
import assert from 'node:assert/strict'
import { queueRows, queuePlaylistIds } from '../src/queue.ts'
import { builtInThemes, captureTheme, savedThemes, validateTheme } from '../src/themePresets.ts'
import { playbackError } from '../src/playbackErrors.ts'
import type { Settings, Track } from '../src/types.ts'

test('queue filtering retains source indices and deduplicates playlist IDs', () => {
  const tracks: Track[] = [{ id: 1, title: 'First' }, { id: 2, title: 'Find me', metadata_artist: 'CUPSIZE' }, { id: 1, title: 'First' }]
  assert.deepEqual(queueRows(tracks, 'cupsize').map(row => row.index), [1])
  assert.deepEqual(queueRows(tracks, 'first').map(row => row.index), [0, 2])
  assert.deepEqual(queuePlaylistIds(tracks), [1, 2])
})

test('themes validate bounds and never export account data or paths', () => {
  for (const preset of builtInThemes) assert.equal(validateTheme(preset), preset)
  const settings = { ...builtInThemes[0].values, client_id: 'SECRET', background_image: 'C:\\Users\\owner\\wallpaper.png' } as unknown as Settings
  const json = JSON.stringify(captureTheme(settings, 'My theme'))
  assert.ok(!json.includes('SECRET'))
  assert.ok(!json.includes('owner'))
  assert.throws(() => validateTheme({ version: 1, name: 'Bad', values: { interface_scale: 9 } }))
  assert.throws(() => validateTheme({ version: 1, name: 'Bad', values: { client_id: 'SECRET' } }))
})

test('playback errors have localized actions without exposing technical payloads', () => {
  const error = 'Playback stopped: probe audio stream https://server/?token=SECRET'
  for (const english of [false, true]) {
    const issue = playbackError(error, english)
    assert.equal(issue.action, 'retry')
    assert.ok(!issue.message.includes('SECRET'))
    assert.ok(!issue.message.includes('https'))
  }
  assert.equal(playbackError('Your SoundCloud session expired', false).action, 'account')
  assert.equal(playbackError('HTTP 403', true).action, 'none')
  assert.equal(playbackError('SoundCloud request limit was reached', true).action, 'none')
})

test('damaged saved themes do not hide valid themes or break Appearance', () => {
  assert.deepEqual(savedThemes([null, { name: { invalid: true } }, builtInThemes[0], builtInThemes[0], builtInThemes[1]]), builtInThemes.slice(0, 2))
  assert.deepEqual(savedThemes(null), [])
})

test('account cache can reset when WebView storage is unavailable', async () => {
  const previous = Object.getOwnPropertyDescriptor(globalThis, 'localStorage')
  Object.defineProperty(globalThis, 'localStorage', { configurable: true, value: {
    getItem() { throw new Error('Storage disabled') },
    setItem() { throw new Error('Storage full') },
    removeItem() { throw new Error('Storage disabled') },
  } })
  try {
    const cache = await import('../src/libraryCache.ts')
    cache.setCacheConnection('signed_in')
    assert.doesNotThrow(() => cache.clearLibraryCache())
    const profile = { status: 'ready' as const, data: { id: 1, username: 'Fixture' } }
    assert.deepEqual(await cache.cachedLibraryData('profile', async () => profile), profile)
    const next = { status: 'ready' as const, data: { id: 2, username: 'Second fixture' } }
    // A fresh account replaces the old snapshot even when storage throws.
    let accountChanged = false
    cache.onLibraryUpdate((_key, _value, changed) => { accountChanged ||= changed })
    await cache.cachedLibraryData('profile', async () => next)
    await new Promise(resolve => setTimeout(resolve, 0))
    assert.equal(accountChanged, true)
    cache.clearLibraryCache()
    cache.setCacheConnection('connecting')
  } finally {
    if (previous) Object.defineProperty(globalThis, 'localStorage', previous)
    else Reflect.deleteProperty(globalThis, 'localStorage')
  }
})
