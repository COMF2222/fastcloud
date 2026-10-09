import { test } from 'node:test'
import assert from 'node:assert/strict'
import { chatActivationInterval, mergeChatMessages, chatError, chatTextParts, type ChatMessage } from '../src/chatTypes.ts'
import { previewChat } from '../src/chatPreview.ts'
import { useChat } from '../src/chatStore.ts'

const row = (id: number): ChatMessage => ({ id, threadId: 1, senderId: 2, text: String(id), nonce: String(id), attachment: null, createdAt: id })
test('message pages and repeated polls merge without duplicates and keep chronological order', () => {
  const messages = mergeChatMessages([row(51), row(52)], [row(50), row(51)])
  assert.deepEqual(messages.map(message => message.id), [50, 51, 52])
  assert.equal(mergeChatMessages(messages, [row(52)]).length, 3)
})
test('chat drafts are isolated by conversation and cleared when the account changes', () => {
  useChat.getState().reset(1)
  useChat.getState().edit(7, { text: 'private', nonce: 'a'.repeat(32) })
  useChat.getState().edit(8, { attachment: { id: 42, kind: 'track', title: 'Song' } })
  assert.equal(useChat.getState().drafts[7].text, 'private')
  assert.equal(useChat.getState().drafts[8].text, '')
  useChat.getState().reset(2)
  assert.deepEqual(useChat.getState().drafts, {})
  assert.equal(useChat.getState().ownerId, 2)
})
test('chat errors give localized actions without exposing server payloads or tokens', () => {
  assert.match(chatError('mutual_required', false), /взаимная подписка/)
  assert.match(chatError('blocked', true), /blocked/)
  for (const english of [false, true]) assert.ok(!chatError('https://server/?token=SECRET', english).includes('SECRET'))
})
test('preview chat retries do not duplicate messages and block/archive behave visibly', async () => {
  const first = await previewChat<ChatMessage>('send', { threadId: 1, text: 'Hello', attachment: null, nonce: 'b'.repeat(32) })
  const second = await previewChat<ChatMessage>('send', { threadId: 1, text: 'Hello', attachment: null, nonce: 'b'.repeat(32) })
  assert.equal(first.id, second.id)
  await previewChat('block', { peerId: 2, blocked: true })
  await assert.rejects(previewChat('send', { threadId: 1, text: 'blocked', nonce: 'c'.repeat(32) }), /blocked/)
  await previewChat('block', { peerId: 2, blocked: false })
  await previewChat('archive', { threadId: 1 })
  assert.equal((await previewChat<{ conversations: unknown[] }>('inbox')).conversations.length, 0)
  await previewChat('open', { peerId: 2 })
  assert.equal((await previewChat<{ conversations: unknown[] }>('inbox')).conversations.length, 1)
})

test('SoundCloud links and profile mentions preserve text and never link unrelated domains', () => {
  const text = 'Hi @https://soundcloud.com/artist, listen https://soundcloud.com/a/b!'
  const parts = chatTextParts(text)
  assert.equal(parts.map(part => part.text).join(''), text)
  assert.deepEqual(parts.filter(part => part.url).map(part => part.url), ['https://soundcloud.com/artist', 'https://soundcloud.com/a/b'])
  assert.equal(chatTextParts('https://soundcloud.com.evil.test/a/b').filter(part => part.url).length, 0)
})


test('chat registration recovers after backend deployment without polling signed-out accounts', () => {
  assert.equal(chatActivationInterval(1, undefined, 'upgrade_backend'), 15_000)
  assert.equal(chatActivationInterval(1, undefined, 'network'), 15_000)
  assert.equal(chatActivationInterval(1, 2, null), 15_000)
  assert.equal(chatActivationInterval(1, 1, null), false)
  assert.equal(chatActivationInterval(0, undefined, null), false)
  assert.equal(chatActivationInterval(1, undefined, 'sign_in'), false)
  assert.equal(chatActivationInterval(1, undefined, 'HTTP 403'), false)
})
