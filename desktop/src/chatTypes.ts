export type ChatPerson = { id: number; username: string; avatarUrl: string | null; permalinkUrl: string | null }
export type ChatAttachment = { id: number; kind: 'track' | 'playlist'; title: string; artist: string; url: string; artworkUrl: string | null; album: boolean }
export type ChatMessage = { id: number; threadId: number; senderId: number; text: string; attachment: ChatAttachment | null; nonce: string; createdAt: number }
export type Conversation = { id: number; peer: ChatPerson; lastMessage: ChatMessage | null; unread: number; blocked: boolean }
export type ChatInbox = { conversations: Conversation[]; unread: number; meId: number }
export type ChatContacts = { contacts: ChatPerson[]; blocked: ChatPerson[]; degraded: boolean }
export type ChatPage = { messages: ChatMessage[]; hasMore: boolean; peerReadId: number; peer: ChatPerson; blocked: boolean; canSend: boolean }
export type ChatAction = 'activate' | 'inbox' | 'contacts' | 'open' | 'messages' | 'send' | 'read' | 'archive' | 'block' | 'report' | 'reports'
export type ChatDraft = { id: number; kind: 'track' | 'playlist'; title: string }

export function mergeChatMessages(current: ChatMessage[], received: ChatMessage[]): ChatMessage[] {
  return [...new Map([...current, ...received].map(message => [message.id, message])).values()].sort((a, b) => a.id - b.id)
}

export function messageNonce(): string { return crypto.randomUUID().replaceAll('-', '') }

export function chatTextParts(text: string): { text: string; url: string | null }[] {
  const parts: { text: string; url: string | null }[] = []
  let at = 0
  for (const match of text.matchAll(/https:\/\/(?:www\.)?soundcloud\.com\/[^\s<>]+/g)) {
    const link = match[0].replace(/[.,;!?)]*$/, '')
    if (match.index > at) parts.push({ text: text.slice(at, match.index), url: null })
    parts.push({ text: link, url: link })
    at = match.index + link.length
  }
  if (at < text.length) parts.push({ text: text.slice(at), url: null })
  return parts.length ? parts : [{ text, url: null }]
}

export function chatError(cause: unknown, english: boolean): string {
  const code = String(cause instanceof Error ? cause.message : cause)
  const messages: Record<string, [string, string]> = {
    sign_in: ['Войди в аккаунт Fastcloud, чтобы открыть сообщения.', 'Sign in to Fastcloud to open messages.'],
    mutual_required: ['Для переписки нужна взаимная подписка в SoundCloud.', 'Mutual SoundCloud following is required to chat.'],
    unavailable: ['Пользователь пока недоступен в чатах Fastcloud.', 'This user is not available in Fastcloud chat yet.'],
    blocked: ['Отправка сообщений заблокирована.', 'Messaging is blocked.'],
    rate_limited: ['Слишком много сообщений. Попробуй чуть позже.', 'Too many messages. Try again shortly.'],
    private_attachment: ['Можно отправлять только публичные треки и плейлисты.', 'Only public tracks and playlists can be shared.'],
    upgrade_backend: ['Чаты ещё не доступны на сервере. Попробуй позже.', 'Chat is not available on the server yet. Try again later.'],
    not_found: ['Диалог не найден.', 'Conversation not found.'],
    invalid: ['Проверь текст сообщения и попробуй ещё раз.', 'Check your message and try again.'],
  }
  return messages[code]?.[english ? 1 : 0] || (english ? 'Could not connect. Try again.' : 'Не удалось подключиться. Попробуй ещё раз.')
}
