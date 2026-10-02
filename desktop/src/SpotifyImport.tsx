import { useEffect, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Download, ExternalLink, FileUp, LoaderCircle } from 'lucide-react'
import { api } from './api'
import type { SpotifyImportKind, SpotifyImportSelection } from './spotifyImportTypes'

export function SpotifyImport({ english }: { english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const queryClient = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [cancelling, setCancelling] = useState(false)
  const [selections, setSelections] = useState<SpotifyImportSelection[]>([])
  const { data: view, error: statusError } = useQuery({ queryKey: ['spotify-import'], queryFn: api.spotifyImportStatus, refetchInterval: query => query.state.data?.running ? 750 : 2500 })
  const wasRunning = useRef(false)
  useEffect(() => {
    if (wasRunning.current && !view?.running) {
      setCancelling(false)
      void queryClient.invalidateQueries({ queryKey: ['settings'] })
      void queryClient.invalidateQueries({ queryKey: ['playlists'] })
      void queryClient.invalidateQueries({ queryKey: ['tracks', 'likes'] })
    }
    wasRunning.current = !!view?.running
  }, [view?.running, queryClient])
  useEffect(() => {
    if (view?.ready) setSelections(view.collections.filter(c => c.tracks > 0).map(c => ({ id: c.id, kind: c.kind })))
  }, [view?.ready, view?.collections])
  const choose = async () => {
    setError(''); setBusy(true)
    try {
      const files = await api.pickSpotifyExport()
      if (files?.length) {
        const parsed = await api.previewSpotifyImport(files)
        queryClient.setQueryData(['spotify-import'], parsed)
      }
    } catch (error) { setError(String(error)) }
    finally { setBusy(false) }
  }
  const start = async () => {
    setError(''); setBusy(true)
    try { await api.startSpotifyImport(selections); await queryClient.invalidateQueries({ queryKey: ['spotify-import'] }) }
    catch (error) { setError(String(error)) }
    finally { setBusy(false) }
  }
  const cancel = async () => {
    setCancelling(true); setError('')
    try { await api.cancelSpotifyImport() }
    catch (error) { setError(String(error)); setCancelling(false) }
  }
  const selectedTracks = view?.collections.filter(c => selections.some(s => s.id === c.id)).reduce((n, c) => n + c.tracks, 0) || 0
  return <div className="settings-card spotify-import">
    <h3>{t('Импорт из Spotify', 'Import from Spotify')}</h3>
    <p className="muted">{t('Перенеси плейлисты и лайки в SoundCloud. Fastcloud найдёт треки автоматически.', 'Move playlists and likes to SoundCloud. Fastcloud matches tracks automatically.')}</p>
    <ol className="spotify-import-steps">
      <li>{t('Открой Exportify, войди в Spotify и нажми Export рядом с плейлистом или Liked Songs. Export All выгрузит плейлисты одним архивом.', 'Open Exportify, sign in to Spotify and click Export next to a playlist or Liked Songs. Export All downloads playlists in one archive.')}</li>
      <li>{t('Выбери скачанный CSV или ZIP ниже и нажми «Импортировать».', 'Choose the downloaded CSV or ZIP below and click Import.')}</li>
    </ol>
    <div className="inline-form">
      <button className="secondary-button" onClick={() => void api.openSpotifyExport().catch(error => setError(String(error)))}><ExternalLink size={16} /> {t('Открыть Exportify', 'Open Exportify')}</button>
      <button className="secondary-button" disabled={busy || view?.running || api.preview} onClick={() => void choose()}>{busy ? <LoaderCircle size={16} className="spin" /> : <FileUp size={16} />} {t('Выбрать выгрузку…', 'Choose export…')}</button>
    </div>
    <p className="muted">{t('Плейлисты будут приватными; лайки попадут в «Любимое». Аудио не переносится — используются версии из SoundCloud.', 'Playlists will be private; likes go to Liked tracks. Audio is not transferred — SoundCloud versions are used.')}</p>
    <details className="advanced-setting"><summary>{t('Другой способ получить файл', 'Another way to get an export')}</summary><p>{t('Поддерживается и официальная выгрузка Spotify: Playlist*.json и YourLibrary.json, отдельно или в ZIP. Запросить её можно в настройках конфиденциальности Spotify.', 'Official Spotify account data is also supported: Playlist*.json and YourLibrary.json, separately or in a ZIP. Request it in Spotify privacy settings.')} <a href="https://www.spotify.com/account/privacy/" target="_blank" rel="noreferrer">Spotify</a></p></details>
    {api.preview && <p className="muted">{t('Выбор файла и импорт доступны в приложении после входа в SoundCloud.', 'File selection and import are available in the desktop app after signing in to SoundCloud.')}</p>}
    {view?.ready && <>
      <div className="spotify-import-collections">
        {view.collections.map(c => {
          const selected = selections.find(s => s.id === c.id)
          return <div key={c.id} className="spotify-import-collection">
            <label><input type="checkbox" disabled={busy || !c.tracks} checked={!!selected} onChange={event => setSelections(values => event.target.checked ? [...values, { id: c.id, kind: c.kind }] : values.filter(s => s.id !== c.id))} /><span><strong>{c.name}</strong><small>{c.tracks} {t('треков', 'tracks')}{c.skipped > 0 && ` · ${t('пропущено', 'skipped')}: ${c.skipped}`}</small></span></label>
            <select aria-label={`${t('Куда импортировать', 'Import destination')}: ${c.name}`} value={selected?.kind || c.kind} disabled={busy || !selected} onChange={event => setSelections(values => values.map(s => s.id === c.id ? { ...s, kind: event.target.value as SpotifyImportKind } : s))}><option value="playlist">{t('Плейлист', 'Playlist')}</option><option value="likes">{t('Любимое', 'Liked tracks')}</option></select>
          </div>
        })}
      </div>
      <button className="secondary-button" disabled={busy || !selectedTracks} onClick={() => void start()}><Download size={16} /> {t('Импортировать', 'Import')} · {selectedTracks} {t('треков', 'tracks')}</button>
    </>}
    {view?.running && <div className="spotify-import-progress" role="status"><progress max={Math.max(1, view.total)} value={view.current} aria-label={t('Ход импорта', 'Import progress')} /><p>{view.current} / {view.total} · {t('найдено', 'matched')}: {view.matched}</p><p className="muted">{view.title}</p><button className="secondary-button" disabled={cancelling} onClick={() => void cancel()}>{cancelling ? t('Останавливаем…', 'Stopping…') : t('Остановить', 'Stop')}</button></div>}
    {(error || statusError || view?.error) && <p className="error-text" role="alert">{error || String(statusError || view?.error)}</p>}
    {view?.cancelled && <p role="status">{t('Импорт остановлен. Уже добавленные треки и плейлисты сохранены.', 'Import stopped. Tracks and playlists already added are kept.')}</p>}
    {!!view?.reports.length && <div className="spotify-import-reports" aria-live="polite">{view.reports.map((report, i) => <div key={i} className="spotify-import-report">
      <strong>{report.name}{!report.completed && !view.running ? ` · ${t('не завершён', 'incomplete')}` : ''}</strong>
      <p>{t('Добавлено', 'Added')}: {report.added}{report.alreadyLiked > 0 && ` · ${t('уже в любимом', 'already liked')}: ${report.alreadyLiked}`} · {t('не найдено', 'not found')}: {report.notFound}{report.playlistIds.length > 1 && ` · ${t('частей плейлиста', 'playlist parts')}: ${report.playlistIds.length}`}</p>
      {report.notFound > 0 && <details><summary>{t('Ненайденные треки', 'Unmatched tracks')}</summary><ul>{report.missing.map((song, index) => <li key={index}>{song}</li>)}</ul>{report.notFound > report.missing.length && <p>{t('Показаны первые', 'Showing the first')} {report.missing.length}.</p>}</details>}
    </div>)}</div>}
  </div>
}
