import type { Data } from './types'

export const soundCloudMessagesUrl = 'https://soundcloud.com/messages'
export type ShareItem = { id: number; title: string; permalink_url?: string | null }
export type ShareKind = 'track' | 'playlist'

export function publicSoundCloudLink(raw: string | null | undefined): string | null {
  if (!raw) return null
  try {
    const url = new URL(raw)
    if (url.protocol !== 'https:' || !['soundcloud.com', 'www.soundcloud.com'].includes(url.hostname)
      || url.username || url.password || url.port || url.searchParams.has('secret_token')
      || url.pathname.split('/').filter(Boolean).length < 2) return null
    url.hostname = 'soundcloud.com'
    url.search = ''; url.hash = ''
    return url.href
  } catch { return null }
}

type Dependencies = {
  detail: (kind: ShareKind, id: number) => Promise<Data<ShareItem>>
  copy: (url: string) => Promise<void>
  open: (url: string) => Promise<void>
  wait: () => Promise<void>
}
export type ShareOutcome = { status: 'opened' | 'copy-failed' | 'open-failed'; url: string } | { status: 'unavailable' }

// SoundCloud does not document a URL for prefilling a message. Copy first, then
// open the inbox; never claim that a message was inserted or sent.
export async function shareSoundCloud(item: ShareItem, kind: ShareKind, deps: Dependencies): Promise<ShareOutcome> {
  let url = publicSoundCloudLink(item.permalink_url)
  if (!url) {
    try {
      for (let attempt = 0; attempt < 12; attempt++) {
        const detail = await deps.detail(kind, item.id)
        if (detail.status === 'ready') { url = publicSoundCloudLink(detail.data.permalink_url); break }
        if (detail.status !== 'loading') break
        await deps.wait()
      }
    } catch { /* Show a localized error rather than remote response contents. */ }
  }
  if (!url) return { status: 'unavailable' }
  try { await deps.copy(url) } catch { return { status: 'copy-failed', url } }
  try { await deps.open(soundCloudMessagesUrl) } catch { return { status: 'open-failed', url } }
  return { status: 'opened', url }
}
