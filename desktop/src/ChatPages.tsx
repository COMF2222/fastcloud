import './chat.css'
import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import * as Dialog from '@radix-ui/react-dialog'
import { Archive, ArrowLeft, Ban, CheckCheck, Flag, LoaderCircle, MessageCircle, Music2, Play, Plus, Send, X } from 'lucide-react'
import { api } from './api'
import { useApp } from './store'
import { useChat } from './chatStore'
import { chatActivationInterval, chatError, chatTextParts, mergeChatMessages, messageNonce, type ChatAttachment, type ChatContacts, type ChatInbox, type ChatMessage, type ChatPage, type ChatPerson } from './chatTypes'
import { SoundCloudShareButton } from './ShareButton'
import { RemoteImage } from './RemoteImage'
import { artist, type Track } from './types'

const emptyDraft = { text: '', attachment: null, nonce: null }

export function useChatSession() {
  const { data: connection } = useQuery({ queryKey: ['connection'], queryFn: api.connection })
  const enabled = api.preview || connection?.status === 'signed_in'
  const { data: me } = useQuery({ queryKey: ['my-profile'], queryFn: api.myProfile, enabled })
  const retainIdentity = enabled || connection?.status === 'error' || connection?.status === 'connecting'
  const id = retainIdentity && me?.status === 'ready' ? me.data.id : 0
  const activation = useQuery({ queryKey: ['chat', id, 'activate'], queryFn: () => api.chat<ChatPerson>('activate'), enabled: enabled && id > 0, staleTime: 10 * 60_000, retry: 1,
    refetchOnWindowFocus: true, refetchOnReconnect: true,
    refetchInterval: query => chatActivationInterval(id, query.state.data?.id, query.state.error) })
  return { id, ready: enabled && id > 0 && activation.data?.id === id, error: activation.error, enabled }
}

function Avatar({ person }: { person: ChatPerson }) {
  return <span className="chat-avatar">{person.avatarUrl ? <RemoteImage src={person.avatarUrl} pixels={100} alt="" /> : person.username.slice(0, 1).toUpperCase()}</span>
}

export function ChatSidebarButton({ english }: { english: boolean }) {
  const session = useChatSession()
  const page = useApp(state => state.page)
  const setPage = useApp(state => state.setPage)
  const queryClient = useQueryClient()
  const previousOwner = useRef(-1)
  useEffect(() => {
    if (previousOwner.current !== session.id) {
      previousOwner.current = session.id; useChat.getState().reset(session.id)
      queryClient.removeQueries({ predicate: query => (query.queryKey[0] === 'chat' || query.queryKey[0] === 'chat-reports') && query.queryKey[1] !== session.id })
    }
  }, [session.id, queryClient])
  const { data } = useQuery({ queryKey: ['chat', session.id, 'inbox'], queryFn: () => api.chat<ChatInbox>('inbox'), enabled: session.ready, retry: 1, refetchInterval: () => document.visibilityState === 'visible' ? page === 'messages' ? false : 15_000 : false })
  return <button className={`nav-item ${page === 'messages' ? 'active' : ''}`} title={english ? 'Messages' : 'Сообщения'} onClick={() => setPage('messages')}><MessageCircle size={19} /><span>{english ? 'Messages' : 'Сообщения'}</span>{!!data?.unread && <small className="chat-badge">{data.unread > 99 ? '99+' : data.unread}</small>}</button>
}

export function ChatProfileButton({ id, english }: { id: number; english: boolean }) {
  const session = useChatSession()
  const { data } = useQuery({ queryKey: ['chat', session.id, 'contacts'], queryFn: () => api.chat<ChatContacts>('contacts'), enabled: session.ready && id !== session.id, staleTime: 60_000, retry: 1 })
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  if (!data?.contacts.some(peer => peer.id === id)) return null
  const open = async () => {
    setBusy(true); setError('')
    try {
      const thread = await api.chat<{ id: number }>('open', { peerId: id })
      if (useChat.getState().ownerId !== session.id) return
      useChat.getState().select(thread.id); useApp.getState().setPage('messages')
    } catch (cause) { setError(chatError(cause, english)) } finally { setBusy(false) }
  }
  return <><button className="secondary-button" disabled={busy} onClick={() => void open()}><MessageCircle size={16} />{english ? 'Message' : 'Написать'}</button>{error && <p role="alert" className="error-text">{error}</p>}</>
}

export function FastcloudShareDialog() {
  const item = useChat(state => state.share)
  const session = useChatSession()
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const { data, isPending, error, refetch } = useQuery({ queryKey: ['chat', session.id, 'contacts'], queryFn: () => api.chat<ChatContacts>('contacts'), enabled: !!item && session.ready, staleTime: 30_000, retry: 1 })
  const [search, setSearch] = useState(''), [busy, setBusy] = useState(false), [issue, setIssue] = useState('')
  useEffect(() => { setSearch(''); setIssue('') }, [item])
  const choose = async (peer: ChatPerson) => {
    if (!item || busy) return
    setBusy(true); setIssue('')
    try {
      const thread = await api.chat<{ id: number }>('open', { peerId: peer.id })
      if (useChat.getState().ownerId !== session.id || useChat.getState().share !== item) return
      useChat.getState().edit(thread.id, { attachment: item, nonce: null })
      useChat.getState().select(thread.id); useChat.getState().sharing(null); useApp.getState().setPage('messages')
    } catch (cause) { setIssue(chatError(cause, !!english)) } finally { setBusy(false) }
  }
  const t = (ru: string, en: string) => english ? en : ru
  const peers = (data?.contacts || []).filter(peer => peer.username.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
  return <Dialog.Root open={!!item} onOpenChange={open => { if (!open) useChat.getState().sharing(null) }}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><Dialog.Content className="chat-share-dialog">
    <Dialog.Title>{t('Поделиться музыкой', 'Share music')}</Dialog.Title><Dialog.Description>{item?.title}</Dialog.Description>
    <input aria-label={t('Найти собеседника', 'Find recipient')} placeholder={t('Найти собеседника…', 'Find recipient…')} value={search} onChange={event => setSearch(event.target.value)} />
    <div className="chat-share-contacts">{session.ready && !error ? isPending ? <LoaderCircle className="spin" /> : peers.map(peer => <button key={peer.id} disabled={busy} onClick={() => void choose(peer)}><Avatar person={peer} /><strong>{peer.username}</strong><Send size={16} /></button>) : <p>{session.error || error ? chatError(session.error || error, !!english) : t('Войди в Fastcloud, чтобы отправить музыку другу.', 'Sign in to Fastcloud to share music with a friend.')}</p>}
      {session.ready && data && !peers.length && <p>{t('Здесь появятся пользователи чатов Fastcloud, с которыми у тебя взаимная подписка в SoundCloud.', 'Fastcloud chat users you mutually follow on SoundCloud will appear here.')}</p>}
      {data?.degraded && <button className="text-button" onClick={() => void refetch()}>{t('Подписки проверены не полностью. Повторить', 'Some follows could not be checked. Retry')}</button>}
    </div>{issue && <p className="error-text" role="alert">{issue}</p>}
    {item && <SoundCloudShareButton item={item} kind={item.kind} english={!!english} label />}
    <Dialog.Close className="icon-button chat-dialog-close" aria-label={t('Закрыть', 'Close')}><X size={18} /></Dialog.Close>
  </Dialog.Content></Dialog.Portal></Dialog.Root>
}

function ChatText({ text, english }: { text: string; english: boolean }) {
  const [error, setError] = useState('')
  const open = async (url: string) => {
    try {
      const item = await api.openLink(url)
      if (item.kind === 'track') useApp.getState().openTrack(item.id, item.title)
      else if (item.kind === 'playlist') useApp.getState().openPlaylist(item.id, item.title)
      else useApp.getState().openArtist(item.id, item.title)
    } catch { setError(english ? 'Could not open the link. Try again.' : 'Не удалось открыть ссылку. Попробуй ещё раз.') }
  }
  return <><p>{chatTextParts(text).map((part, index) => part.url ? <button className="chat-url" key={index} onClick={() => void open(part.url!)}>{part.text}</button> : part.text)}</p>{error && <p className="error-text" role="alert">{error}</p>}</>
}

function MusicCard({ item, english }: { item: ChatAttachment; english: boolean }) {
  const [busy, setBusy] = useState(false), [error, setError] = useState('')
  const queryClient = useQueryClient()
  const open = () => item.kind === 'track' ? useApp.getState().openTrack(item.id, item.title) : useApp.getState().openPlaylist(item.id, item.title)
  const play = async () => {
    setBusy(true); setError('')
    try {
      for (let attempt = 0; attempt < 12; attempt++) {
        const result = item.kind === 'track' ? await api.track(item.id) : await api.tracks('playlist', undefined, item.id)
        if (result.status === 'ready') {
          const tracks = Array.isArray(result.data) ? result.data : [result.data]
          if (!tracks.length) throw Error('empty')
          await api.play(tracks, 0); await queryClient.invalidateQueries({ queryKey: ['player'] }); return
        }
        if (result.status !== 'loading') throw Error('unavailable')
        await new Promise(resolve => setTimeout(resolve, 300))
      }
      throw Error('network')
    } catch { setError(english ? 'Could not play. Open the music page and try again.' : 'Не удалось включить. Открой страницу музыки и попробуй ещё раз.') } finally { setBusy(false) }
  }
  return <div className="chat-music-card"><button className="chat-music-open" onClick={open}>{item.artworkUrl ? <RemoteImage src={item.artworkUrl} pixels={100} alt="" /> : <span className="chat-music-art"><Music2 size={24} /></span>}<span><small>{item.kind === 'track' ? english ? 'TRACK' : 'ТРЕК' : item.album ? english ? 'ALBUM' : 'АЛЬБОМ' : english ? 'PLAYLIST' : 'ПЛЕЙЛИСТ'}</small><strong>{item.title}</strong><small>{item.artist}</small></span></button><button className="icon-button" disabled={busy} aria-label={english ? `Play ${item.title}` : `Слушать ${item.title}`} onClick={() => void play()}>{busy ? <LoaderCircle className="spin" size={18} /> : <Play size={18} />}</button>{error && <p role="alert">{error}</p>}</div>
}

export function MessagesPage() {
  const session = useChatSession()
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const threadId = useChat(state => state.threadId), drafts = useChat(state => state.drafts)
  const draft = threadId ? drafts[threadId] || emptyDraft : emptyDraft
  const queryClient = useQueryClient()
  const inbox = useQuery({ queryKey: ['chat', session.id, 'inbox'], queryFn: () => api.chat<ChatInbox>('inbox'), enabled: session.ready, refetchInterval: () => document.visibilityState === 'visible' ? 3000 : false, retry: 1 })
  const contacts = useQuery({ queryKey: ['chat', session.id, 'contacts'], queryFn: () => api.chat<ChatContacts>('contacts'), enabled: session.ready, refetchInterval: 60_000, staleTime: 30_000, retry: 1 })
  const [search, setSearch] = useState(''), [issue, setIssue] = useState(''), [sending, setSending] = useState(false), [olderBusy, setOlderBusy] = useState(false)
  const [musicOpen, setMusicOpen] = useState(false), [reportOpen, setReportOpen] = useState(false)
  const [feed, setFeed] = useState<{ owner: number; thread: number | null; rows: ChatMessage[]; hasOlder: boolean }>({ owner: 0, thread: null, rows: [], hasOlder: false })
  const cursor = useRef({ owner: 0, thread: null as number | null, after: 0 })
  const scroller = useRef<HTMLDivElement>(null), stick = useRef(true), prepend = useRef<number | null>(null)
  const read = useRef(0)
  const composer = useRef<HTMLTextAreaElement>(null), composerFocusPending = useRef(true)
  const page = useQuery({ queryKey: ['chat', session.id, 'messages', threadId], enabled: session.ready && !!threadId, retry: 1,
    queryFn: async () => {
      const after = cursor.current.owner === session.id && cursor.current.thread === threadId ? cursor.current.after : 0
      return { page: await api.chat<ChatPage>('messages', { threadId, after }), initial: after === 0, owner: session.id, thread: threadId }
    }, refetchInterval: () => document.visibilityState === 'visible' ? 3000 : false })
  useEffect(() => { composerFocusPending.current = true }, [threadId, session.id])
  useEffect(() => {
    if (!composerFocusPending.current || !threadId || !session.ready || !page.data?.page.canSend || sending || musicOpen || reportOpen) return
    const frame = requestAnimationFrame(() => {
      const element = composer.current
      if (!element || element.disabled || document.visibilityState !== 'visible') return
      composerFocusPending.current = false
      // A user who started typing in search while the conversation loaded
      // keeps that focus. Polling never takes focus back from another control.
      const focused = document.activeElement
      if (focused !== element && focused?.matches('input, textarea, [contenteditable="true"]')) return
      element.focus({ preventScroll: true })
    })
    return () => cancelAnimationFrame(frame)
  }, [threadId, session.id, session.ready, page.data?.page.canSend, sending, musicOpen, reportOpen])
  useEffect(() => { setIssue(''); stick.current = true; read.current = 0 }, [threadId, session.id])
  useEffect(() => { setSending(false); setOlderBusy(false) }, [session.id])
  useEffect(() => {
    const received = page.data
    if (!received || received.thread !== threadId || received.owner !== session.id) return
    setFeed(previous => {
      const same = previous.owner === session.id && previous.thread === threadId
      const rows = mergeChatMessages(same ? previous.rows : [], received.page.messages)
      const previousCursor = cursor.current.owner === session.id && cursor.current.thread === threadId ? cursor.current.after : 0
      cursor.current = { owner: session.id, thread: threadId, after: Math.max(previousCursor, received.page.messages.at(-1)?.id || 0) }
      return { owner: session.id, thread: threadId, rows, hasOlder: received.initial ? received.page.hasMore : same && previous.hasOlder }
    })
  }, [page.data, session.id, threadId])
  const rows = feed.owner === session.id && feed.thread === threadId ? feed.rows : []
  const markRead = () => {
    const last = cursor.current.owner === session.id && cursor.current.thread === threadId ? cursor.current.after : 0
    if (!threadId || !last || last <= read.current || !stick.current || document.visibilityState !== 'visible' || !document.hasFocus()) return
    read.current = last
    void api.chat('read', { threadId, through: last }).then(() => queryClient.invalidateQueries({ queryKey: ['chat', session.id, 'inbox'] })).catch(() => { read.current = 0 })
  }
  useLayoutEffect(() => {
    const element = scroller.current
    if (!element) return
    if (prepend.current !== null) { element.scrollTop += element.scrollHeight - prepend.current; prepend.current = null }
    else if (stick.current) element.scrollTop = element.scrollHeight
    markRead()
  }, [rows])
  useEffect(() => { window.addEventListener('focus', markRead); return () => window.removeEventListener('focus', markRead) }, [rows, threadId, session.id])
  const peer = page.data?.page.peer || inbox.data?.conversations.find(item => item.id === threadId)?.peer
  const selfBlocked = !!peer && contacts.data?.blocked.some(item => item.id === peer.id)
  const invalidate = () => Promise.all(['inbox', 'messages', 'contacts'].map(section => queryClient.invalidateQueries({ queryKey: ['chat', session.id, section] })))
  const selectConversation = (next: number) => {
    const same = useChat.getState().threadId === next
    useChat.getState().select(next)
    if (same) composer.current?.focus({ preventScroll: true })
  }
  const choose = async (person: ChatPerson) => {
    setIssue('')
    try { const next = await api.chat<{ id: number }>('open', { peerId: person.id }); if (useChat.getState().ownerId !== session.id) return; selectConversation(next.id); await invalidate() }
    catch (cause) { setIssue(chatError(cause, !!english)) }
  }
  const send = async () => {
    if (!threadId || sending || !page.data?.page.canSend || (!draft.text.trim() && !draft.attachment)) return
    const target = threadId, snapshot = { ...draft, nonce: draft.nonce || messageNonce() }
    useChat.getState().edit(target, { nonce: snapshot.nonce }); setSending(true); setIssue('')
    try {
      const message = await api.chat<ChatMessage>('send', { threadId: target, text: snapshot.text, nonce: snapshot.nonce, attachment: snapshot.attachment ? { kind: snapshot.attachment.kind, id: snapshot.attachment.id } : null })
      if (useChat.getState().ownerId !== session.id) return
      if (useChat.getState().drafts[target]?.nonce === snapshot.nonce) useChat.getState().edit(target, { text: '', attachment: null, nonce: null })
      stick.current = true
      setFeed(previous => previous.thread === target && previous.owner === session.id ? { ...previous, rows: mergeChatMessages(previous.rows, [message]) } : previous)
      await invalidate()
    } catch (cause) { setIssue(chatError(cause, !!english)) } finally {
      if (useChat.getState().ownerId === session.id && useChat.getState().threadId === target) composerFocusPending.current = true
      setSending(false)
    }
  }
  const older = async () => {
    if (!threadId || !rows.length || olderBusy) return
    setOlderBusy(true)
    const target = threadId
    try {
      const result = await api.chat<ChatPage>('messages', { threadId: target, before: rows[0].id })
      if (useChat.getState().ownerId !== session.id || useChat.getState().threadId !== target) return
      prepend.current = scroller.current?.scrollHeight || 0
      setFeed(previous => previous.thread === target ? { ...previous, rows: mergeChatMessages(previous.rows, result.messages), hasOlder: result.hasMore } : previous)
    } catch (cause) { setIssue(chatError(cause, !!english)) } finally { setOlderBusy(false) }
  }
  const action = async (type: 'archive' | 'block' | 'report', reason?: string) => {
    if (!threadId || !peer) return
    try {
      await api.chat(type, type === 'block' ? { peerId: peer.id, blocked: !selfBlocked } : { threadId, ...(reason ? { reason } : {}) })
      if (useChat.getState().ownerId !== session.id) return
      if (type !== 'block') useChat.getState().select(null)
      setReportOpen(false); await invalidate()
    } catch (cause) { setIssue(chatError(cause, !!english)) }
  }
  const active = threadId && session.ready
  const visibleConversations = (inbox.data?.conversations || []).filter(item => item.peer.username.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
  const visibleContacts = (contacts.data?.contacts || []).filter(person => person.username.toLocaleLowerCase().includes(search.toLocaleLowerCase()))
  const uploads = useQuery({ queryKey: ['tracks', 'uploads'], queryFn: () => api.tracks('uploads'), enabled: musicOpen, refetchInterval: result => result.state.data?.status === 'loading' ? 1200 : false })
  const likes = useQuery({ queryKey: ['tracks', 'likes', undefined, undefined], queryFn: () => api.tracks('likes'), enabled: musicOpen, refetchInterval: result => result.state.data?.status === 'loading' ? 1200 : false })
  const [musicSearch, setMusicSearch] = useState('')
  const music = [...new Map([...(uploads.data?.status === 'ready' ? uploads.data.data : []), ...(likes.data?.status === 'ready' ? likes.data.data : [])].map(track => [track.id, track])).values()].filter(track => [track.title, artist(track)].some(value => value.toLocaleLowerCase().includes(musicSearch.toLocaleLowerCase())))
  const attach = (track: Track) => { if (threadId) useChat.getState().edit(threadId, { attachment: { id: track.id, kind: 'track', title: track.title }, nonce: null }); setMusicOpen(false) }
  return <div className="page-content messages-page"><div className="messages-heading"><h1>{t('Сообщения', 'Messages')}</h1><p>{t('Чаты Fastcloud с друзьями по взаимной подписке SoundCloud.', 'Fastcloud chats with friends you mutually follow on SoundCloud.')}</p></div>
    {!session.ready && <div className="chat-empty">{session.error ? chatError(session.error, !!english) : session.enabled ? t('Подключаем сообщения…', 'Connecting messages…') : session.id ? t('Чаты временно недоступны. Попробуй ещё раз.', 'Chat is temporarily unavailable. Try again.') : t('Войди в аккаунт, чтобы открыть сообщения.', 'Sign in to open messages.')}</div>}
    {session.ready && <div className={`chat-layout ${active ? 'chat-selected' : ''}`}><aside className="chat-list"><input aria-label={t('Поиск диалогов и друзей', 'Search conversations and friends')} placeholder={t('Найти друга…', 'Find a friend…')} value={search} onChange={event => setSearch(event.target.value)} />
      {inbox.error && <p role="alert">{chatError(inbox.error, !!english)}</p>}{visibleConversations.map(item => <button className={`chat-list-person ${item.id === threadId ? 'active' : ''}`} key={item.id} onClick={() => selectConversation(item.id)}><Avatar person={item.peer} /><span><strong>{item.peer.username}</strong><small>{item.lastMessage?.text || item.lastMessage?.attachment?.title || t('Новый диалог', 'New conversation')}</small></span>{!!item.unread && <b className="chat-badge">{item.unread}</b>}</button>)}
      <h3>{t('Начать диалог', 'Start a conversation')}</h3>{contacts.isPending ? <LoaderCircle className="spin" /> : visibleContacts.map(person => <button className="chat-list-person" key={person.id} onClick={() => void choose(person)}><Avatar person={person} /><span><strong>{person.username}</strong><small>Fastcloud</small></span><Plus size={16} /></button>)}
      {!visibleContacts.length && !contacts.isPending && <p>{t('Здесь появятся друзья, с которыми вы взаимно подписаны в SoundCloud. Для переписки обоим нужен обновлённый Fastcloud.', 'Friends you mutually follow on SoundCloud will appear here. Both of you need an up-to-date Fastcloud app to chat.')}</p>}{(contacts.error || contacts.data?.degraded) && <button className="text-button" onClick={() => void contacts.refetch()}>{t('Не удалось проверить все подписки. Повторить', 'Could not check all follows. Retry')}</button>}
      {!!contacts.data?.blocked.length && <details><summary>{t('Заблокированные', 'Blocked users')}</summary>{contacts.data.blocked.map(person => <button key={person.id} className="text-button" onClick={() => void api.chat('block', { peerId: person.id, blocked: false }).then(invalidate).catch(cause => setIssue(chatError(cause, !!english)))}>{person.username} · {t('Разблокировать', 'Unblock')}</button>)}</details>}
    </aside><section className="chat-conversation">{active ? <>
      <header className="chat-header"><button className="icon-button chat-back" aria-label={t('Все диалоги', 'All conversations')} onClick={() => useChat.getState().select(null)}><ArrowLeft size={17} /></button><button className="chat-peer" onClick={() => peer && useApp.getState().openArtist(peer.id, peer.username)}>{peer && <Avatar person={peer} />}<strong>{peer?.username || t('Диалог', 'Conversation')}</strong></button><div><button className="icon-button" title={t('Убрать диалог в архив', 'Archive conversation')} aria-label={t('Убрать диалог в архив', 'Archive conversation')} onClick={() => void action('archive')}><Archive size={17} /></button><button className="icon-button" title={selfBlocked ? t('Разблокировать', 'Unblock') : t('Заблокировать', 'Block')} aria-label={selfBlocked ? t('Разблокировать', 'Unblock') : t('Заблокировать', 'Block')} onClick={() => void action('block')}><Ban size={17} /></button><button className="icon-button" title={t('Пожаловаться', 'Report')} aria-label={t('Пожаловаться', 'Report')} onClick={() => setReportOpen(true)}><Flag size={17} /></button></div></header>
      <div className="chat-history" ref={scroller} aria-label={t('История диалога', 'Conversation history')} onScroll={() => { const el = scroller.current; if (el) stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 70; markRead() }}>
        {feed.hasOlder && rows.length > 0 && <button className="text-button chat-older" disabled={olderBusy} onClick={() => void older()}>{t('Ранние сообщения', 'Earlier messages')}</button>}{page.isPending && <LoaderCircle className="spin" />}{page.error && <p role="alert">{chatError(page.error, !!english)}</p>}
        {rows.map(row => <article key={row.id} className={`chat-message ${row.senderId === session.id ? 'mine' : ''}`}><div className="chat-bubble">{row.text && <ChatText text={row.text} english={!!english} />}{row.attachment && <MusicCard item={row.attachment} english={!!english} />}<small className="chat-message-time">{new Date(row.createdAt).toLocaleString(english ? 'en-US' : 'ru-RU', { month: 'short', day: 'numeric', hour: '2-digit', minute: '2-digit' })}{row.senderId === session.id && <span title={row.id <= (page.data?.page.peerReadId || 0) ? t('Прочитано', 'Read') : t('Отправлено', 'Sent')}>{row.id <= (page.data?.page.peerReadId || 0) ? <CheckCheck size={13} /> : t('Отправлено', 'Sent')}</span>}</small></div></article>)}
      </div><form className="chat-compose" onSubmit={event => { event.preventDefault(); void send() }}>{draft.attachment && <div className="chat-draft-music"><Music2 size={17} /><strong>{draft.attachment.title}</strong><button type="button" className="icon-button" disabled={sending} aria-label={t('Убрать вложение', 'Remove attachment')} onClick={() => threadId && useChat.getState().edit(threadId, { attachment: null, nonce: null })}><X size={16} /></button></div>}
        {page.data && !page.data.page.canSend && <p>{t('Чтобы отправлять сообщения, нужна взаимная подписка и доступ к чату.', 'Mutual following and chat access are required to send messages.')}</p>}<div className="chat-compose-row"><button type="button" className="icon-button" disabled={sending || !page.data?.page.canSend} aria-label={t('Добавить музыку', 'Add music')} title={t('Добавить музыку', 'Add music')} onClick={() => setMusicOpen(true)}><Music2 size={19} /></button><textarea ref={composer} disabled={sending || !page.data?.page.canSend} maxLength={4000} aria-label={t('Сообщение', 'Message')} placeholder={t('Напиши сообщение…', 'Write a message…')} value={draft.text} onChange={event => threadId && useChat.getState().edit(threadId, { text: event.target.value, nonce: null })} onKeyDown={event => { if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) { event.preventDefault(); void send() } }} /><button className="primary-button" type="submit" disabled={sending || !page.data?.page.canSend || (!draft.text.trim() && !draft.attachment)} aria-label={t('Отправить', 'Send')}>{sending ? <LoaderCircle className="spin" size={18} /> : <Send size={18} />}</button></div></form>
    </> : <div className="chat-empty"><MessageCircle size={38} /><p>{t('Выбери диалог или друга, чтобы начать общение.', 'Choose a conversation or a friend to start chatting.')}</p></div>}</section></div>}{issue && <p className="error-text" role="alert">{issue}</p>}
    <Dialog.Root open={musicOpen} onOpenChange={setMusicOpen}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><Dialog.Content className="chat-share-dialog" onCloseAutoFocus={event => { event.preventDefault(); composer.current?.focus({ preventScroll: true }) }}><Dialog.Title>{t('Добавить музыку', 'Add music')}</Dialog.Title><Dialog.Description>{t('Твои лайки и загрузки', 'Your likes and uploads')}</Dialog.Description><input aria-label={t('Найти музыку для отправки', 'Find music to share')} value={musicSearch} onChange={event => setMusicSearch(event.target.value)} placeholder={t('Найти трек…', 'Find a track…')} /><div className="chat-music-picker">{music.slice(0, 100).map(track => <button key={track.id} onClick={() => attach(track)}><Music2 size={18} /><span><strong>{track.title}</strong><small>{artist(track)}</small></span><Plus size={16} /></button>)}{!music.length && <p>{t('Лайкни или загрузи трек — он появится здесь.', 'Like or upload a track to see it here.')}</p>}</div><Dialog.Close className="icon-button chat-dialog-close" aria-label={t('Закрыть', 'Close')}><X size={18} /></Dialog.Close></Dialog.Content></Dialog.Portal></Dialog.Root>
    <Dialog.Root open={reportOpen} onOpenChange={setReportOpen}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><Dialog.Content className="chat-share-dialog"><Dialog.Title>{t('Пожаловаться на диалог', 'Report conversation')}</Dialog.Title><Dialog.Description>{t('Собеседник будет заблокирован, а диалог убран в архив. Последние сообщения из диалога попадут в жалобу администратору Fastcloud.', 'The user will be blocked and the conversation archived. Recent messages will be included in the report to the Fastcloud administrator.')}</Dialog.Description><div className="chat-report-actions"><button className="secondary-button" onClick={() => void action('report', 'spam')}>{t('Спам', 'Spam')}</button><button className="secondary-button" onClick={() => void action('report', 'harassment')}>{t('Оскорбления', 'Harassment')}</button></div><Dialog.Close className="icon-button chat-dialog-close" aria-label={t('Закрыть', 'Close')}><X size={18} /></Dialog.Close></Dialog.Content></Dialog.Portal></Dialog.Root>
  </div>
}
