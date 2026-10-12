import { useEffect, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from './api'

export function ImportRecovery({ english }: { english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const client = useQueryClient()
  const { data: view } = useQuery({ queryKey: ['spotify-import'], queryFn: api.spotifyImportStatus, refetchInterval: query => query.state.data?.running ? 750 : 2500 })
  const [token, setToken] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const wasRunning = useRef(false)
  useEffect(() => {
    if (wasRunning.current && !view?.running) {
      for (const key of [['settings'], ['playlists'], ['tracks', 'likes'], ['yandex-import']]) void client.invalidateQueries({ queryKey: key })
    }
    wasRunning.current = !!view?.running
  }, [view?.running, client])
  if (!view || view.ready || !view.source) return null
  const act = async (action: () => Promise<void>) => {
    setBusy(true); setError('')
    try { await action(); setToken(''); await client.invalidateQueries({ queryKey: ['spotify-import'] }) }
    catch (error) { setError(String(error)) }
    finally { setBusy(false) }
  }
  return <section className="settings-card">
    <h3>{view.resumable || view.running ? t('Сохранённый импорт', 'Saved import') : t('Импорт завершён', 'Import complete')} · {view.source === 'yandex' ? t('Яндекс Музыка', 'Yandex Music') : 'Spotify'}</h3>
    <p>{t('Прогресс сохраняется. Можно продолжить импорт или снова поискать только ненайденные треки.', 'Progress is saved. Resume your import or search again for unmatched tracks.')}</p>
    {view.needsToken && <label className="field-label">{t('Токен Яндекс Музыки для оставшихся треков', 'Yandex Music token for the remaining tracks')}<input type="password" autoComplete="off" value={token} onChange={event => setToken(event.target.value)} /></label>}
    {view.running ? <><p role="status">{view.current} / {view.total} · {view.title}</p><button className="secondary-button" disabled={busy} onClick={() => void act(api.cancelSpotifyImport)}>{t('Остановить', 'Stop')}</button></> : <div className="inline-form">
      {view.resumable && <button className="secondary-button" disabled={busy || (view.needsToken && !token.trim())} onClick={() => void act(() => api.resumeImport(false, token || null))}>{t('Продолжить импорт', 'Resume import')}</button>}
      {view.retryMissing && <button className="secondary-button" disabled={busy || (view.needsToken && !token.trim())} onClick={() => void act(() => api.resumeImport(true, token || null))}>{t('Повторить поиск ненайденных', 'Retry unmatched tracks')}</button>}
      <button className="secondary-button" disabled={busy} onClick={() => { if (window.confirm(t('Удалить сохранённый прогресс? Уже добавленные плейлисты и лайки останутся в SoundCloud.', 'Discard saved progress? Playlists and likes already added will stay in SoundCloud.'))) void act(api.discardImport) }}>{t('Убрать сохранённый импорт', 'Discard saved import')}</button>
    </div>}
    {(error || (view.source === 'yandex' && view.error)) && <p role="alert" className="error-text">{error || view.error}</p>}
    {view.source === 'yandex' && view.reports.map((report, index) => <div key={index} className="spotify-import-report"><strong>{report.name}</strong><p>{t('Добавлено', 'Added')}: {report.added} · {t('Не найдено', 'Not found')}: {report.notFound}</p>{!!report.missing.length && <details><summary>{t('Ненайденные треки', 'Unmatched tracks')}</summary><ul>{report.missing.map((name, index) => <li key={index}>{name}</li>)}</ul></details>}</div>)}
  </section>
}
