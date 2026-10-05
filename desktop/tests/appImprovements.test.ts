import { test } from 'node:test'
import assert from 'node:assert/strict'
import { queueRows, queuePlaylistIds } from '../src/queue.ts'
import { builtInThemes, captureTheme, savedThemes, themeIsActive, validateTheme } from '../src/themePresets.ts'
import { contrast, headingSurfaceOpacity, luminance, panelPalette, readableAccent } from '../src/theme.ts'
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

test('ready-made palettes preserve user sizes and selection tolerates native float precision', () => {
  const settings = { ...builtInThemes[3].values, panel_opacity: .85000002384, interface_text_scale: 1.25 } as Settings
  assert.ok(themeIsActive(settings, builtInThemes[3]))
  for (const preset of builtInThemes) {
    assert.equal(preset.values.interface_text_scale, undefined)
    assert.equal(preset.values.interface_scale, undefined)
  }
})

test('opaque surfaces stay in the selected theme and all surface levels retain contrast', () => {
  for (const light of [false, true]) for (const color of [[181, 156, 225], [244, 237, 222], [23, 20, 34], [117, 117, 117], [255, 0, 0], [0, 255, 0], [0, 0, 255], [255, 255, 255], [0, 0, 0]]) {
    const palette = panelPalette(color, 1, light)
    assert.ok(light ? luminance(palette.surface) >= .7 : luminance(palette.surface) <= .07)
    for (const background of palette.backgrounds) {
      assert.ok(contrast(background, palette.text) >= 4.5)
      assert.ok(contrast(background, palette.muted) >= 4.5)
      assert.ok(contrast(background, palette.controlLine) >= 3)
    }
    assert.ok(contrast(palette.background, readableAccent([184, 156, 255], palette.background, palette.text)) >= 4.5)
    assert.equal(palette.alpha, 1)
    assert.notDeepEqual(palette.nav, palette.raised)
  }
  assert.equal(panelPalette([23, 20, 34], 0, false).alpha, 0)
  assert.deepEqual(panelPalette([181,156,225],.1,false).surface,[181,156,225])
})

test('surface tint changes continuously and automatic headings avoid a second panel', () => {
  let previous = panelPalette([181,156,225],0,false).surface
  for (let index = 1; index <= 100; index++) {
    const surface = panelPalette([181,156,225],index/100,false).surface
    assert.ok(surface.every((channel, i) => Math.abs(channel - previous[i]) <= 5))
    previous = surface
  }
  const settings = { panel_rgb:[181,156,225],panel_opacity:1,heading_opacity:null } as Settings
  assert.equal(headingSurfaceOpacity(settings),0)
  assert.equal(headingSurfaceOpacity({ ...settings, heading_opacity:.6 }),.6)
  for (const light of [false,true]) {
    let previous = panelPalette([0,0,0],1,light).surface
    for (let channel = 1; channel <= 255; channel++) {
      const surface = panelPalette([channel,channel,channel],1,light).surface
      assert.ok(surface.every((value,i) => Math.abs(value - previous[i]) <= 4))
      previous = surface
    }
  }
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
