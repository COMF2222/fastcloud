import { artist, type Track } from './types'

export { parseLrc, currentLyricLine, type LyricLine } from './lyricsTiming'

export function lyricsIdentity(track: Track, durationMs: number) {
  const performer = artist(track)
  // Keep recording-version labels intact; the native matcher cleans metadata.
  const title = track.title.trim()
  const album = track.publisher_metadata?.album_title || null
  const isrc = track.publisher_metadata?.isrc || null
  return { performer, title, album, isrc, durationMs,
    key: ['track-lyrics', track.id, performer, title, Math.round(durationMs / 1000), album, isrc] as const }
}
