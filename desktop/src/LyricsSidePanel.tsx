import { useEffect, useMemo, useRef } from 'react'
import { useQuery } from '@tanstack/react-query'
import { AudioLines, LoaderCircle, Music2 } from 'lucide-react'
import { api } from './api'
import { cleanTitle, parseLrc } from './lyrics'
import { artist, type Settings } from './types'

export function LyricsSidePanel({ settings }: { settings?: Settings }) {
  const english = settings?.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const { data: player } = useQuery({ queryKey: ['player'], queryFn: api.player, refetchInterval: 500, retry: false })
  const track = player?.current == null ? null : player.queue[player.current]
  const trackArtist = track ? artist(track) : ''
  const length = track ? Math.max(1, player?.durationMs || track.full_duration_ms || track.duration || 1) : 1
  const { data: record, isPending, isError, refetch } = useQuery({
    queryKey: ['track-lyrics', track?.id, trackArtist, cleanTitle(track?.title || ''), Math.round(length / 1000)],
    queryFn: () => api.trackLyrics(trackArtist, cleanTitle(track!.title), length),
    enabled: !!track,
    staleTime: 60 * 60_000,
    retry: false,
  })
  const lines = useMemo(() => record?.syncedLyrics ? parseLrc(record.syncedLyrics) : [], [record?.syncedLyrics])
  let currentLine = -1
  for (let index = 0; index < lines.length && lines[index].time <= (player?.positionMs || 0); index++) currentLine = index
  const scroll = useRef<HTMLDivElement>(null)
  const active = useRef<HTMLButtonElement>(null)
  useEffect(() => { scroll.current?.scrollTo({ top: 0 }) }, [track?.id])
  useEffect(() => {
    if (settings?.lyrics_auto_scroll === false) return
    const container = scroll.current
    const line = active.current
    if (!container || !line) return
    const offset = line.getBoundingClientRect().top - container.getBoundingClientRect().top
    container.scrollTo({ top: Math.max(0, container.scrollTop + offset - container.clientHeight * .34), behavior: settings?.reduced_motion ? 'auto' : 'smooth' })
  }, [currentLine, track?.id, settings?.reduced_motion, settings?.lyrics_auto_scroll])
  return <aside id="lyrics-panel" className="lyrics-panel" aria-label={t('Текст песни', 'Lyrics')} style={{ '--lyrics-scale': settings?.lyrics_scale || 1 } as React.CSSProperties}>
    <header className="lyrics-panel-header"><span><AudioLines size={18} /> {t('Текст песни', 'Lyrics')}</span></header>
    {track ? <>
      <div className="lyrics-panel-scroll" ref={scroll}>{isPending ? <p className="lyrics-panel-message"><LoaderCircle className="spin" size={18} /> {t('Ищем текст…', 'Finding lyrics…')}</p> : isError ? <div className="lyrics-panel-message"><p>{t('Не удалось загрузить текст.', 'Could not load lyrics.')}</p><button className="secondary-button" onClick={() => void refetch()}>{t('Повторить', 'Retry')}</button></div> : record?.instrumental ? <p className="lyrics-panel-message">{t('Инструментальный трек', 'Instrumental track')}</p> : lines.length ? <div className={`lyrics-panel-lines ${settings?.lyrics_blur_past ? 'blur-past' : ''}`}>{lines.map((line, index) => <button key={`${line.time}-${index}`} ref={index === currentLine ? active : undefined} className={index === currentLine ? 'active' : index < currentLine ? 'past' : ''} onClick={() => void api.transport('seek', line.time)} title={t('Перейти к строке', 'Jump to line')}>{line.text || '♪'}</button>)}</div> : record?.plainLyrics ? <div className="lyrics-panel-plain">{record.plainLyrics}</div> : <p className="lyrics-panel-message">{t('Текст пока не найден.', 'Lyrics are not available yet.')}</p>}</div>
      {record && <small className="lyrics-panel-source">{t('Источник', 'Source')}: {record.source || 'LRCLIB'}</small>}
    </> : <p className="lyrics-panel-message"><Music2 size={22} /> {t('Включи трек, чтобы увидеть текст.', 'Play a track to see its lyrics.')}</p>}
  </aside>
}
