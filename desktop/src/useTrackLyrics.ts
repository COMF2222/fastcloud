import { useQuery } from '@tanstack/react-query'
import { api } from './api'
import { currentLyricLine, lyricsIdentity, type LyricLine } from './lyrics'
import type { PlayerState, Track } from './types'

/** Both lyrics views share the same enriched metadata and result. */
export function useTrackLyrics(track: Track | null, durationMs: number, enabled = true) {
  const needsDetail = enabled && !!track && !track.publisher_metadata?.isrc
  const detail = useQuery({ queryKey: ['track', track?.id], queryFn: () => api.track(track!.id),
    enabled: needsDetail, staleTime: 60 * 60_000, retry: false,
    refetchInterval: query => query.state.data?.status === 'loading' ? 1200 : false })
  const identified = detail.data?.status === 'ready' && detail.data.data.id === track?.id ? detail.data.data : track
  const identity = identified ? lyricsIdentity(identified, durationMs) : null
  const waiting = needsDetail && (detail.isPending || detail.data?.status === 'loading')
  const lyrics = useQuery({ queryKey: identity?.key || ['track-lyrics', null],
    queryFn: () => api.trackLyrics(identity!.performer, identity!.title, durationMs, identity!.album, identity!.isrc),
    enabled: enabled && !!identity && !waiting, staleTime: 60 * 60_000, retry: false })
  return lyrics
}

/** Only timed lyrics need the faster clock; unrelated player UI keeps its cadence. */
export function useLyricsLine(lines: LyricLine[], state: PlayerState | undefined) {
  const trackId = state?.current == null ? null : state.queue[state.current]?.id
  const running = lines.length > 0 && !!state?.isPlaying && !state.loading
  const { data: line } = useQuery({ queryKey: ['lyrics-playback'], queryFn: api.player,
    enabled: running, refetchInterval: 100, retry: false,
    select: sample => sample.current != null && sample.queue[sample.current]?.id === trackId
      ? currentLyricLine(lines, sample.positionMs) : null })
  return running && line != null ? line : currentLyricLine(lines, state?.positionMs || 0)
}
