export type SpotifyImportKind = 'playlist' | 'likes'
export type SpotifyImportSelection = { id: number; kind: SpotifyImportKind }
export interface SpotifyImportView {
  resumable: boolean
  retryMissing: boolean
  needsToken: boolean
  source: string
  running: boolean
  ready: boolean
  current: number
  total: number
  matched: number
  title: string
  error: string
  cancelled: boolean
  collections: { id: number; name: string; kind: SpotifyImportKind; tracks: number; skipped: number }[]
  reports: { name: string; matched: number; added: number; alreadyLiked: number; notFound: number; missing: string[]; playlistIds: number[]; completed: boolean }[]
}
export const emptySpotifyImport: SpotifyImportView = { resumable: false, retryMissing: false, needsToken: false, source: '', running: false, ready: false, current: 0, total: 0, matched: 0, title: '', error: '', cancelled: false, collections: [], reports: [] }
