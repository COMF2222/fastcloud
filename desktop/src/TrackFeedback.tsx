import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Heart, Play, Undo2 } from 'lucide-react'
import { api } from './api'
import { ShareButton } from './ShareButton'
import { Artwork, Empty } from './App'
import { artist, duration, type Track } from './types'
import { useApp } from './store'
import { useVirtualRows } from './useVirtualRows'

export function DislikedTracks({ filter = '' }: { filter?: string }) {
  const queryClient = useQueryClient()
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const { data, isPending, error } = useQuery({ queryKey: ['wave-dislikes'], queryFn: api.waveDislikes, refetchInterval: 10_000 })
  const [busy, setBusy] = useState<number | null>(null)
  const [actionError, setActionError] = useState('')
  const search = filter.trim().toLocaleLowerCase()
  const tracks = (data || []).filter(track => !search || [track.title, artist(track)].some(value => value.toLocaleLowerCase().includes(search)))
  const rowHeight = 78 * (settings?.interface_scale || 1) * (settings?.interface_text_scale || 1)
  const virtual = useVirtualRows(tracks.length, rowHeight, tracks.length > 200)
  const restore = async (track: Track, like = false) => {
    if (busy !== null) return
    setBusy(track.id); setActionError('')
    try {
      if (like) await api.setLiked(track.id, true)
      else await api.waveDislike(track, false)
      await Promise.all(['wave-dislikes', 'wave-disliked', 'player', 'settings', 'tracks'].map(key => queryClient.invalidateQueries({ queryKey: [key] })))
    } catch (cause) { setActionError(String(cause)) }
    finally { setBusy(null) }
  }
  const play = async (track: Track) => {
    try { await api.play([track], 0); await queryClient.invalidateQueries({ queryKey: ['player'] }) }
    catch (cause) { setActionError(String(cause)) }
  }
  if (isPending) return <p className="muted">{t('Загружаем дизлайки…', 'Loading dislikes…')}</p>
  if (error) return <p className="error-text" role="alert">{String(error)}</p>
  return <div className="disliked-tracks">
    <p className="muted">{t('Эти треки исключены из рекомендаций. Дизлайк можно убрать или заменить лайком.', 'These tracks are excluded from recommendations. Remove a dislike or replace it with a like.')}</p>
    {actionError && <p className="error-text" role="alert">{actionError}</p>}
    {!tracks.length ? <Empty message={search ? t('Ничего не найдено', 'Nothing found') : t('Дизлайков пока нет', 'No disliked tracks')} /> :
      <div ref={virtual.ref} className="disliked-list">
        {virtual.start > 0 && <div style={{ height: virtual.start * rowHeight }} aria-hidden="true" />}
        {tracks.slice(virtual.start, virtual.end).map(track => <article className="disliked-row" key={track.id} style={{ height: rowHeight }}>
          <button className="disliked-cover" onClick={() => void play(track)} aria-label={`${t('Слушать', 'Play')} ${track.title}`}><Artwork item={track} /><Play size={16} /></button>
          <button className="disliked-info" onClick={() => useApp.getState().openTrack(track.id, track.title)}><strong>{track.title}</strong><small>{artist(track)} · {duration(track.full_duration_ms || track.duration || 0)}</small></button>
          <div className="disliked-actions"><ShareButton item={track} english={!!english} /><button className="secondary-button" disabled={busy !== null} onClick={() => void restore(track)} title={t('Убрать дизлайк', 'Remove dislike')}><Undo2 size={16} /><span>{t('Убрать дизлайк', 'Remove dislike')}</span></button><button className="icon-button" disabled={busy !== null} onClick={() => void restore(track, true)} aria-label={t('Убрать дизлайк и лайкнуть', 'Remove dislike and like')} title={t('Убрать дизлайк и лайкнуть', 'Remove dislike and like')}><Heart size={18} /></button></div>
        </article>)}
        {virtual.end < tracks.length && <div style={{ height: (tracks.length - virtual.end) * rowHeight }} aria-hidden="true" />}
      </div>}
  </div>
}
