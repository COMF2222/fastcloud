import { test } from 'node:test'
import assert from 'node:assert/strict'
import { publicSoundCloudLink, shareSoundCloud, soundCloudMessagesUrl } from '../src/soundcloudShare.ts'

test('shared links are canonical SoundCloud pages without tracking or credentials', () => {
  assert.equal(publicSoundCloudLink('https://www.soundcloud.com/lil_peep/white-wine?utm_source=id_1#comments'), 'https://soundcloud.com/lil_peep/white-wine')
  for (const raw of ['javascript:alert(1)', 'https://soundcloud.com.evil.test/a/b', 'https://soundcloud.com@evil.test/a/b', 'https://user:secret@soundcloud.com/a/b', 'http://soundcloud.com/a/b', 'https://soundcloud.com/a/b?secret_token=secret', 'https://soundcloud.com/messages']) assert.equal(publicSoundCloudLink(raw), null)
})

test('sharing copies before opening messages and never invents prefill parameters', async () => {
  const calls: string[] = []
  const result = await shareSoundCloud({ id: 1, title: 'Song', permalink_url: 'https://soundcloud.com/a/b?utm_source=app' }, 'track', {
    detail: async () => { throw Error('Metadata must not be fetched when the link is present') },
    copy: async url => { calls.push(`copy:${url}`) },
    open: async url => { calls.push(`open:${url}`) },
    wait: async () => {},
  })
  assert.equal(result.status, 'opened')
  assert.deepEqual(calls, ['copy:https://soundcloud.com/a/b', `open:${soundCloudMessagesUrl}`])
})

test('an album or playlist with sparse metadata resolves its own link, not a track link', async () => {
  let requests = 0
  const calls: string[] = []
  const result = await shareSoundCloud({ id: 42, title: 'Album' }, 'playlist', {
    detail: async (kind, id) => {
      assert.equal(kind, 'playlist'); assert.equal(id, 42)
      return ++requests === 1 ? { status: 'loading' } : { status: 'ready', data: { id, title: 'Album', permalink_url: 'https://soundcloud.com/a/sets/album' } }
    },
    copy: async url => { calls.push(url) }, open: async url => { calls.push(url) }, wait: async () => {},
  })
  assert.equal(result.status, 'opened')
  assert.deepEqual(calls, ['https://soundcloud.com/a/sets/album', soundCloudMessagesUrl])
})

test('clipboard and browser failures retain a usable link and report the failed step', async () => {
  for (const failure of ['copy', 'open']) {
    let opened = false
    const result = await shareSoundCloud({ id: 1, title: 'Song', permalink_url: 'https://soundcloud.com/a/b' }, 'track', {
      detail: async () => ({ status: 'unavailable' }),
      copy: async () => { if (failure === 'copy') throw Error('Permission denied') },
      open: async () => { opened = true; throw Error('Browser missing') }, wait: async () => {},
    })
    assert.equal(result.status, failure === 'copy' ? 'copy-failed' : 'open-failed')
    assert.equal(opened, failure === 'open')
    assert.ok('url' in result && result.url === 'https://soundcloud.com/a/b')
  }
})

test('unavailable or persistently loading metadata never copies or opens an unrelated URL', async () => {
  for (const loading of [false, true]) {
    let requests = 0
    const result = await shareSoundCloud({ id: 1, title: 'Song' }, 'track', {
      detail: async () => { requests++; return { status: loading ? 'loading' : 'unavailable' } },
      copy: async () => { assert.fail('No link to copy') }, open: async () => { assert.fail('No link to share') }, wait: async () => {},
    })
    assert.equal(result.status, 'unavailable')
    assert.equal(requests, loading ? 12 : 1)
  }
})
