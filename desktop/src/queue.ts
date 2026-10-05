import type { Track } from './types'
export function queueRows(tracks: Track[], query: string) {
  const needle = query.trim().normalize('NFKC').toLocaleLowerCase()
  return tracks.map((track, index) => ({ track, index })).filter(({ track }) =>
    !needle || `${track.title} ${track.publisher_metadata?.artist || track.metadata_artist || track.user?.username || ''}`.normalize('NFKC').toLocaleLowerCase().includes(needle))
}
export function queuePlaylistIds(tracks: Track[]) { return [...new Set(tracks.map(track => track.id))] }
