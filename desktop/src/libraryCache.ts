import type { Connection, Data, Me } from './types'

const storageKey = 'fastcloud:library-snapshot:v2'
// Rebuild derived API snapshots on upgrade. Sessions, playlists, preferences and
// downloads are owned by the native app and are not stored in this cache.
try { localStorage.removeItem('fastcloud:library-snapshot:v1') } catch { /* Storage can be unavailable. */ }
type Snapshot = { accountId: number; entries: Record<string, Data<unknown>> }
let snapshot: Snapshot | null = (() => {
  try {
    const value = JSON.parse(localStorage.getItem(storageKey) || 'null') as Snapshot | null
    return value && Number.isSafeInteger(value.accountId) && value.entries && typeof value.entries === 'object' && !Array.isArray(value.entries) && (value.entries.profile?.status !== 'ready' || (value.entries.profile.data as Me)?.id === value.accountId) ? value : null
  } catch { return null }
})()
let connection: Connection['status'] = 'connecting'
let listener: ((key: string, data: Data<unknown>, accountChanged: boolean) => void) | null = null
const pending = new Map<string, Data<unknown>>()
const refreshing = new Set<string>()
const versions = new Map<string, number>()
let generation = 0

export function setCacheConnection(status: Connection['status']) { connection = status }
export function onLibraryUpdate(callback: (key: string, data: Data<unknown>, accountChanged: boolean) => void) { listener = callback }

export function clearLibraryCache() {
  generation++
  snapshot = null
  pending.clear()
  try { localStorage.removeItem(storageKey) }
  catch { /* Clearing the in-memory account cache must also work without storage. */ }
}

export function forgetLibraryEntry(key: string) {
  versions.set(key, (versions.get(key) || 0) + 1)
  pending.delete(key)
  if (!snapshot) return
  delete snapshot.entries[key]
  try { localStorage.setItem(storageKey, JSON.stringify(snapshot)) }
  catch { /* The running library can still load from SoundCloud. */ }
}

function persist<T>(key: string, value: Data<T>) {
  if (value.status !== 'ready') return
  if (key === 'profile') {
    const id = (value.data as Me).id
    if (!Number.isSafeInteger(id)) return
    const changed = snapshot !== null && snapshot.accountId !== id
    if (changed) clearLibraryCache()
    snapshot ||= { accountId: id, entries: {} }
    if (!changed) for (const [name, entry] of pending) snapshot.entries[name] = entry
    pending.clear()
    snapshot.entries.profile = value
    if (changed) listener?.(key, value, true)
  } else if (snapshot) snapshot.entries[key] = value
  else { pending.set(key, value); return }
  try { localStorage.setItem(storageKey, JSON.stringify(snapshot)) }
  catch { /* A full WebView storage must never break browsing. */ }
}

function refresh<T>(key: string, request: () => Promise<Data<T>>) {
  if (refreshing.has(key)) return
  const started = generation
  const version = versions.get(key) || 0
  refreshing.add(key)
  void (async () => {
    try {
      for (let attempt = 0; attempt < 40 && connection === 'signed_in' && started === generation && version === (versions.get(key) || 0); attempt++) {
        const value = await request()
        if (started !== generation || version !== (versions.get(key) || 0) || connection !== 'signed_in') return
        if (value.status === 'ready') {
          persist(key, value)
          listener?.(key, value, false)
          return
        }
        if (value.status === 'failed' || value.status === 'unavailable') break
        await new Promise(resolve => setTimeout(resolve, 400))
      }
    } catch { /* Keep the last successful snapshot during a network outage. */ }
  })().finally(() => {
    refreshing.delete(key)
  })
}

export async function cachedLibraryData<T>(key: string, request: () => Promise<Data<T>>): Promise<Data<T>> {
  const started = generation
  const version = versions.get(key) || 0
  const old = connection === 'signed_in' ? snapshot?.entries[key] as Data<T> | undefined : undefined
  if (old?.status === 'ready') {
    refresh(key, request)
    return old
  }
  const value = await request()
  if (connection === 'signed_in' && started === generation && version === (versions.get(key) || 0)) persist(key, value)
  return value
}
