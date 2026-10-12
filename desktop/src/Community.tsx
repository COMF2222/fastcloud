import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { Trophy } from 'lucide-react'
import { api } from './api'
import type { LeaderboardEntry, Settings } from './types'

const hours = (ms: number, english: boolean) => `${(ms / 3600000).toLocaleString(english ? 'en-US' : 'ru-RU', { maximumFractionDigits: 1 })} ${english ? 'h' : 'ч'}`

export function WaveModes({ settings, english }: { settings: Settings; english: boolean }) {
  const client = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const choose = async (key: 'wave_mode' | 'wave_mood', value: string) => {
    setBusy(true); setError('')
    try { await api.setSetting(key, value); await client.invalidateQueries({ queryKey: ['settings'] }) }
    catch (error) { setError(String(error)) }
    finally { setBusy(false) }
  }
  return <div className="wave-mode-controls">
    <label>{english ? 'Wave mode' : 'Режим волны'}<select disabled={busy} value={settings.wave_mode || 'Balanced'} onChange={e => void choose('wave_mode', e.target.value)}>
      {([['Balanced','По вкусу','Your taste'],['Familiar','Привычное','Familiar'],['Discover','Открытия','Discoveries'],['Mood','Под настроение','By mood'],['NoRepeats','Без повторов · 7 дней','No repeats · 7 days']] as const).map(([value, ru, en]) => <option key={value} value={value}>{english ? en : ru}</option>)}
    </select></label>
    {settings.wave_mode === 'Mood' && <label>{english ? 'Mood' : 'Настроение'}<select disabled={busy} value={settings.wave_mood || 'Calm'} onChange={e => void choose('wave_mood', e.target.value)}>
      {([['Calm','Спокойное','Calm'],['Sad','Грустное','Sad'],['Happy','Весёлое','Happy'],['Workout','Тренировка','Workout'],['Road','В дороге','On the road']] as const).map(([value, ru, en]) => <option key={value} value={value}>{english ? en : ru}</option>)}
    </select></label>}
    {error && <p className="error-text" role="alert">{error}</p>}
  </div>
}

export function LeaderboardPrivacy({ settings, banner = false }: { settings: Settings; banner?: boolean }) {
  const english = settings.language === 'English'
  const client = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const toggle = async (anonymous: boolean) => {
    setBusy(true); setError('')
    try { await api.leaderboardAnonymity(anonymous); await client.invalidateQueries({ queryKey: ['settings'] }); await client.invalidateQueries({ queryKey: ['leaderboard'] }) }
    catch (error) { setError(String(error)) }
    finally { setBusy(false) }
  }
  return <div className={`settings-card ${banner ? 'leaderboard-privacy-banner' : ''}`}><h3>{banner ? (settings.leaderboard_anonymous ?? true) ? (english ? 'You appear anonymously' : 'Ты участвуешь анонимно') : (english ? 'Your name is visible' : 'Твой ник виден в рейтинге') : english ? 'Leaderboard privacy' : 'Приватность лидерборда'}</h3>
    <label className="setting-row"><span><strong>{english ? 'Anonymous in the leaderboard' : 'Анонимность в лидерборде'}</strong><small>{(settings.leaderboard_anonymous ?? true) ? english ? 'Your listening still counts. Other participants see “Anonymous” instead of your name.' : 'Прослушивания учитываются. Другие участники видят «Аноним» вместо твоего ника.' : english ? 'Other participants see your name. Enable anonymity to hide it.' : 'Другие участники видят твой ник. Включи анонимность, чтобы скрыть его.'}</small></span><input type="checkbox" checked={settings.leaderboard_anonymous ?? true} disabled={busy} onChange={e => void toggle(e.target.checked)} /></label>
    {error && <p className="error-text" role="alert">{error}</p>}
  </div>
}

export function LeaderboardPage({ english }: { english: boolean }) {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const [days, setDays] = useState(0)
  const [sort, setSort] = useState('time')
  const t = (ru: string, en: string) => english ? en : ru
  const { data, error, isPending } = useQuery({ queryKey: ['leaderboard', days, sort], queryFn: () => api.leaderboard(days, sort), refetchInterval: 15000 })
  const row = (entry: LeaderboardEntry) => <article className={`leaderboard-row ${entry.isMe ? 'is-me' : ''}`} key={entry.rank}>
    <strong className="leaderboard-rank">{entry.rank}</strong>
    <div className="leaderboard-person"><strong>{entry.anonymous ? t(`Аноним #${entry.rank}`, `Anonymous #${entry.rank}`) : entry.name}{entry.isMe && <small className="leaderboard-you">{t('Ты', 'You')}</small>}</strong>
      <span>{entry.artists.map(artist => artist.name).join(' · ') || t('Исполнители пока не указаны', 'No artist details yet')}</span>
      {!!entry.tracks.length && <details><summary>{t('Что слушает', 'Listening to')}</summary>{entry.tracks.map((track, index) => <p key={index}>{track.artist} — {track.title} <small>{hours(track.ms, english)}</small></p>)}</details>}
    </div>
    <div className="leaderboard-metric"><strong>{hours(entry.ms, english)}</strong><small>{t('прослушано', 'listening time')}</small></div>
    <div className="leaderboard-metric"><strong>{entry.plays.toLocaleString()}</strong><small>{t('прослушиваний', 'listens')}</small><small>{entry.uniqueTracks} {t('разных треков', 'unique tracks')}</small></div>
  </article>
  return <div className="page-content community-page"><div className="section-title"><div><h2>{t('Лидерборд', 'Leaderboard')}</h2><p>{t('Кто больше слушает музыку в Fastcloud.', 'Who listens the most in Fastcloud.')}</p></div><Trophy size={28} /></div>
    {settings && <LeaderboardPrivacy settings={settings} banner />}
    <div className="community-filters"><label className="field-label">{t('Период', 'Period')}<select value={days} onChange={e => setDays(Number(e.target.value))}><option value={7}>{t('7 дней', '7 days')}</option><option value={30}>{t('30 дней', '30 days')}</option><option value={0}>{t('Всё время', 'All time')}</option></select></label><label className="field-label">{t('Рейтинг по', 'Rank by')}<select value={sort} onChange={e => setSort(e.target.value)}><option value="time">{t('Времени прослушивания', 'Listening time')}</option><option value="plays">{t('Числу прослушиваний', 'Listens')}</option></select></label></div>
    {error && <p role="alert" className="error-text">{String(error)}</p>}
    {isPending && <p role="status">{t('Загружаем рейтинг…', 'Loading leaderboard…')}</p>}
    {data && <><p className="muted">{data.participants} {t('участников · даты в UTC', 'participants · dates in UTC')}</p><section className="settings-card leaderboard-list">{data.entries.map(row)}{!data.entries.length && <p>{t('Начни слушать — после синхронизации здесь появится твоя статистика.', 'Start listening. Your statistics will appear after syncing.')}</p>}</section>{data.mine && !data.entries.some(entry => entry.isMe) && <section className="settings-card"><h3>{t('Твоё место', 'Your position')}</h3>{row(data.mine)}</section>}</>}
  </div>
}
