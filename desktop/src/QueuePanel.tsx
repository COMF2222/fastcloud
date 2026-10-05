import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import * as Dialog from '@radix-ui/react-dialog'
import { ArrowDown, ArrowUp, AudioLines, Music2, X } from 'lucide-react'
import { RemoteImage } from './RemoteImage'
import { api } from './api'
import { artist, cover, type PlayerState } from './types'
import { queuePlaylistIds, queueRows } from './queue'

export function QueuePanel({ state, english, action }: { state?: PlayerState; english: boolean; action: (action: string, value?: number, index?: number, target?: number) => Promise<void> }) {
  const t = (ru: string, en: string) => english ? en : ru
  const client = useQueryClient()
  const [search, setSearch] = useState('')
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const rows = queueRows(state?.queue || [], search)
  const save = async () => {
    const ids = queuePlaylistIds(state?.queue || [])
    if (!name.trim() || !ids.length) return
    setBusy(true); setMessage('')
    try {
      await api.saveQueuePlaylist(name.trim(), ids)
      await client.invalidateQueries({ queryKey: ['playlists'] })
      setName(''); setMessage(t('Очередь сохранена в приватный плейлист.', 'Queue saved as a private playlist.'))
    } catch (e) { setMessage(String(e)) }
    finally { setBusy(false) }
  }
  return <><div className="queue-heading"><div><Dialog.Title>{t('Очередь воспроизведения', 'Playback queue')}</Dialog.Title><Dialog.Description>{state?.queue.length || 0} {t('треков', 'tracks')}</Dialog.Description></div><Dialog.Close className="icon-button" aria-label={t('Закрыть очередь', 'Close queue')}><X size={20} /></Dialog.Close></div>
    <input className="queue-search" aria-label={t('Поиск в очереди', 'Search queue')} placeholder={t('Трек или исполнитель…', 'Track or artist…')} value={search} onChange={e => setSearch(e.target.value)} />
    <div className="queue-undo"><button className="text-button" disabled={!state?.canUndoQueue} onClick={() => void action('undo_queue')}>{t('Отменить изменение очереди', 'Undo queue change')}</button></div>
    <div className="queue-toolbar"><button disabled={state?.current == null} onClick={() => void action('clear_played')}>{t('Убрать прослушанное', 'Clear played')}</button><button disabled={!state?.queue.length} onClick={() => void action('clear_upcoming')}>{t('Очистить следующие', 'Clear upcoming')}</button><button disabled={!state?.queue.length} onClick={() => void action('clear_queue')}>{t('Очистить всё', 'Clear all')}</button></div>
    <form className="inline-form queue-save" onSubmit={e => { e.preventDefault(); void save() }}><input value={name} maxLength={128} aria-label={t('Название плейлиста очереди', 'Queue playlist name')} placeholder={t('Сохранить всю очередь как…', 'Save the whole queue as…')} onChange={e => setName(e.target.value)} /><button disabled={busy || !name.trim() || !state?.queue.length}>{t('Сохранить', 'Save')}</button></form>{message && <p role="status">{message}</p>}
    <div className="queue-items">{rows.map(({ track, index }) => <div key={`${track.id}-${index}`} className={`queue-item ${index === state?.current ? 'playing' : ''}`}><button onClick={() => void action('skip_to', undefined, index)}><span className="art-row">{cover(track) ? <RemoteImage src={cover(track)!} pixels={200} alt="" /> : <Music2 size={18} />}</span><span><strong>{track.title}</strong><small>{artist(track)}</small></span></button><div className="queue-order"><button className="icon-button" disabled={index === 0} aria-label={t('Поднять', 'Move up') + ' ' + track.title} onClick={() => void action('move', undefined, index, index - 1)}><ArrowUp size={14} /></button><button className="icon-button" disabled={index === (state?.queue.length || 0) - 1} aria-label={t('Опустить', 'Move down') + ' ' + track.title} onClick={() => void action('move', undefined, index, index + 1)}><ArrowDown size={14} /></button></div>{index === state?.current ? <AudioLines size={17} /> : <button className="icon-button queue-remove" aria-label={t('Удалить', 'Remove') + ': ' + track.title} onClick={() => void action('remove', undefined, index)}><X size={17} /></button>}</div>)}{!rows.length && <p className="muted">{search ? t('Ничего не найдено', 'No matches') : t('Очередь пуста', 'Queue is empty')}</p>}</div>
  </>
}
