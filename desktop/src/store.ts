import { create } from 'zustand'

export type Page = 'home' | 'discover' | 'catalog' | 'search' | 'feed' | 'library' | 'offline' | 'likes' | 'history' | 'reposts' | 'inbox' | 'settings' | 'playlist' | 'artist' | 'track'
type Location = { page: Page; search: string; searchKind: 'tracks' | 'artists'; playlistId: number | null; playlistTitle: string; artistId: number | null; artistName: string; trackId: number | null; trackTitle: string }
type AppStore = Location & {
  artistLookup: string | null
  queueOpen: boolean
  uploadPath: string | null
  libraryTab: 'overview' | 'tracks' | 'playlists' | 'liked_playlists' | 'albums' | 'uploads' | 'stations' | 'artists' | 'history'
  history: Location[]
  historyIndex: number
  setPage: (page: Page) => void
  setSearch: (search: string) => void
  openArtistLookup: (name: string) => void
  clearArtistLookup: () => void
  openPlaylist: (id: number, title: string) => void
  renamePlaylist: (id: number, title: string) => void
  openArtist: (id: number, name: string) => void
  openTrack: (id: number, title: string) => void
  goBack: () => void
  goForward: () => void
  setQueueOpen: (open: boolean) => void
  setUploadPath: (path: string | null) => void
  setLibraryTab: (tab: 'overview' | 'tracks' | 'playlists' | 'liked_playlists' | 'albums' | 'uploads' | 'stations' | 'artists' | 'history') => void
}

const defaultLocation: Location = { page: 'home', search: '', searchKind: 'tracks', playlistId: null, playlistTitle: '', artistId: null, artistName: '', trackId: null, trackTitle: '' }
const navigationKey = 'fastcloud:last-location:v1'
const pages: Page[] = ['home', 'discover', 'catalog', 'search', 'feed', 'library', 'offline', 'likes', 'history', 'reposts', 'inbox', 'settings', 'playlist', 'artist', 'track']
const libraryTabs: AppStore['libraryTab'][] = ['overview', 'tracks', 'playlists', 'liked_playlists', 'albums', 'uploads', 'stations', 'artists', 'history']
function savedNavigation(): { location: Location; libraryTab: AppStore['libraryTab'] } | null {
  try {
    const saved = JSON.parse(localStorage.getItem(navigationKey) || 'null')
    if (!saved || !pages.includes(saved.page)) return null
    if (saved.page === 'playlist' && !(saved.playlistId > 0)) return null
    if (saved.page === 'artist' && !(saved.artistId > 0)) return null
    if (saved.page === 'track' && !(saved.trackId > 0)) return null
    return {
      location: {
        page: saved.page,
        search: typeof saved.search === 'string' ? saved.search : '',
        searchKind: saved.searchKind === 'artists' ? 'artists' : 'tracks',
        playlistId: Number.isSafeInteger(saved.playlistId) ? saved.playlistId : null,
        playlistTitle: typeof saved.playlistTitle === 'string' ? saved.playlistTitle : '',
        artistId: Number.isSafeInteger(saved.artistId) ? saved.artistId : null,
        artistName: typeof saved.artistName === 'string' ? saved.artistName : '',
        trackId: Number.isSafeInteger(saved.trackId) ? saved.trackId : null,
        trackTitle: typeof saved.trackTitle === 'string' ? saved.trackTitle : '',
      },
      libraryTab: libraryTabs.includes(saved.libraryTab) ? saved.libraryTab : 'overview',
    }
  } catch { return null }
}
const restored = savedNavigation()
export const hasRestoredNavigation = !!restored
const initial = restored?.location || defaultLocation
const location = (state: AppStore): Location => ({ page: state.page, search: state.search, searchKind: state.searchKind, playlistId: state.playlistId, playlistTitle: state.playlistTitle, artistId: state.artistId, artistName: state.artistName, trackId: state.trackId, trackTitle: state.trackTitle })
const navigate = (state: AppStore, next: Location) => ({
  ...next,
  history: [...state.history.slice(0, state.historyIndex + 1), next],
  historyIndex: state.historyIndex + 1,
})

export const useApp = create<AppStore>((set) => ({
  ...initial, artistLookup: null, queueOpen: false, uploadPath: null, libraryTab: restored?.libraryTab || 'overview', history: [initial], historyIndex: 0,
  setPage: page => set(state => state.page === page ? state : navigate(state, { ...location(state), page })),
  setSearch: search => set(state => {
    const next = { ...location(state), page: 'search' as Page, search, searchKind: 'tracks' as const }
    if (state.page === 'search') {
      const history = [...state.history]
      history[state.historyIndex] = next
      return { ...next, history, artistLookup: null }
    }
    return { ...navigate(state, next), artistLookup: null }
  }),
  openArtistLookup: name => set(state => ({ ...navigate(state, { ...location(state), page: 'search', search: name, searchKind: 'artists' }), artistLookup: name })),
  clearArtistLookup: () => set({ artistLookup: null }),
  openPlaylist: (playlistId, playlistTitle) => set(state => navigate(state, { ...location(state), page: 'playlist', playlistId, playlistTitle })),
  renamePlaylist: (id, title) => set(state => ({
    playlistTitle: state.playlistId === id ? title : state.playlistTitle,
    history: state.history.map(item => item.playlistId === id ? { ...item, playlistTitle: title } : item),
  })),
  openArtist: (artistId, artistName) => set(state => ({ ...navigate(state, { ...location(state), page: 'artist', artistId, artistName }), artistLookup: null })),
  openTrack: (trackId, trackTitle) => set(state => navigate(state, { ...location(state), page: 'track', trackId, trackTitle })),
  goBack: () => set(state => state.historyIndex > 0 ? { ...state.history[state.historyIndex - 1], historyIndex: state.historyIndex - 1 } : state),
  goForward: () => set(state => state.historyIndex < state.history.length - 1 ? { ...state.history[state.historyIndex + 1], historyIndex: state.historyIndex + 1 } : state),
  setQueueOpen: queueOpen => set({ queueOpen }),
  setUploadPath: uploadPath => set({ uploadPath }),
  setLibraryTab: libraryTab => set({ libraryTab }),
}))

let lastNavigation = ''
useApp.subscribe(state => {
  const value = JSON.stringify({ ...location(state), libraryTab: state.libraryTab })
  if (value === lastNavigation) return
  lastNavigation = value
  try { localStorage.setItem(navigationKey, value) } catch { /* Storage may be unavailable. */ }
})
