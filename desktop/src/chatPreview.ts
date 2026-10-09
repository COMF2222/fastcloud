import type { ChatAction, ChatAttachment, ChatMessage, ChatPerson } from './chatTypes'

const peer: ChatPerson = { id: 2, username: 'Luna Waves · демо', avatarUrl: null, permalinkUrl: null }
const me: ChatPerson = { id: 1, username: 'SoundCloud Demo', avatarUrl: null, permalinkUrl: null }
let rows: ChatMessage[] = [{ id: 1, threadId: 1, senderId: 2, text: 'Привет! Здесь можно проверить чаты Fastcloud. Это демонстрационный диалог.', attachment: null, nonce: 'a'.repeat(32), createdAt: Date.now() - 60_000 }]
let archived = false, blocked = false, read = 0

export async function previewChat<T>(action: ChatAction, input: Record<string, unknown> = {}): Promise<T> {
  let result: unknown = { ok: true }
  if (action === 'activate') result = me
  if (action === 'contacts') result = { contacts: blocked ? [] : [peer], blocked: blocked ? [peer] : [], degraded: false }
  if (action === 'inbox') {
    const unread = rows.filter(row => row.senderId !== 1 && row.id > read).length
    result = { meId: 1, unread: archived ? 0 : unread, conversations: archived ? [] : [{ id: 1, peer, lastMessage: rows.at(-1), unread, blocked }] }
  }
  if (action === 'open') { if (blocked) throw Error('blocked'); if (input.peerId !== peer.id) throw Error('unavailable'); archived = false; result = { id: 1, peer } }
  if (action === 'messages') result = { messages: rows.filter(row => (!input.after || row.id > Number(input.after)) && (!input.before || row.id < Number(input.before))), hasMore: false, peerReadId: 0, peer, blocked, canSend: !blocked }
  if (action === 'send') {
    if (blocked) throw Error('blocked')
    const previous = rows.find(row => row.nonce === input.nonce)
    if (previous) result = previous
    else {
      const raw = input.attachment as { id: number; kind: 'track' | 'playlist' } | null
      const attachment: ChatAttachment | null = raw ? { ...raw, title: raw.kind === 'track' ? ['Northern Lights', 'Sunset Drive', 'Rain on Glass', 'Neon District', 'Paper Planes', 'Golden Hour', 'Static Fields', 'Low Tide', 'Concrete Garden', 'Afterglow'][raw.id - 1000] || 'Demo track' : ['Neon Nights', 'Low Tide Radio', 'Concrete Garden Mix'][raw.id - 2001] || 'Demo playlist', artist: 'SoundCloud Demo', artworkUrl: null, album: raw.kind === 'playlist' && raw.id === 2001, url: 'https://soundcloud.com/' } : null
      const next = { id: (rows.at(-1)?.id || 0) + 1, threadId: 1, senderId: 1, text: String(input.text || ''), attachment, nonce: String(input.nonce), createdAt: Date.now() }
      rows = [...rows, next]; result = next; archived = false
    }
  }
  if (action === 'read') read = Math.max(read, Number(input.through))
  if (action === 'archive' || action === 'report') { archived = true; read = rows.at(-1)?.id || 0 }
  if (action === 'block') blocked = !!input.blocked
  if (action === 'report') blocked = true
  if (action === 'reports') result = { reports: [] }
  return result as T
}
