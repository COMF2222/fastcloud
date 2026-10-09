import { create } from 'zustand'
import type { ChatDraft } from './chatTypes'

// Drafts are session-local: no private chat text enters portable settings or logs.
type Draft = { text: string; attachment: ChatDraft | null; nonce: string | null }
export const useChat = create<{
  ownerId: number; threadId: number | null; share: ChatDraft | null; drafts: Record<number, Draft>
  select: (id: number | null) => void; sharing: (item: ChatDraft | null) => void
  edit: (id: number, draft: Partial<Draft>) => void; reset: (ownerId?: number) => void
}>((set) => ({
  ownerId: 0, threadId: null, share: null, drafts: {},
  select: threadId => set({ threadId }), sharing: share => set({ share }),
  edit: (id, draft) => set(state => ({ drafts: { ...state.drafts, [id]: { ...(state.drafts[id] || { text: '', attachment: null, nonce: null }), ...draft } } })),
  reset: (ownerId = 0) => set({ ownerId, threadId: null, share: null, drafts: {} }),
}))
