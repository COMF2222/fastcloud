import { useEffect, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import * as Dialog from '@radix-ui/react-dialog'
import { X } from 'lucide-react'
import { api } from './api'
import { useApp } from './store'
import type { MusicTaste, ProblemReport, Settings } from './types'
import { builtInThemes, savedThemes, type ThemePreset } from './themePresets'
import { playbackError } from './playbackErrors'

export function TasteSettings({ settings, update }: { settings: Settings; update: (key: string, value: unknown) => Promise<void> }) {
  const t = (ru: string, en: string) => settings.language === 'English' ? en : ru
  const [draft, setDraft] = useState<MusicTaste>(settings.music_taste)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => setDraft(settings.music_taste), [settings.music_taste])
  const genres = ['Ambient', 'Electronic', 'Hip-hop & Rap', 'Alternative Rock', 'Pop', 'Indie', 'House', 'Phonk', 'Jazz', 'R&B', 'Techno', 'Drum & Bass']
  return <div className="settings-card improvement-card"><h3>{t('Музыкальный вкус', 'Musical taste')}</h3><p className="muted">{t('Настройки влияют на следующие треки «Моей волны». Лайки и дизлайки продолжают учитываться.', 'These preferences affect upcoming My Wave recommendations. Likes and dislikes still count.')}</p>
    <label className="preference-range"><span>{t('Знакомое → новое', 'Familiar → new')} <output>{Math.round(draft.discovery * 100)}%</output></span><input type="range" min="0" max="1" step=".05" value={draft.discovery} onChange={e => setDraft({ ...draft, discovery: Number(e.target.value) })} /></label>
    <label className="preference-range"><span>{t('Разнообразие исполнителей', 'Artist diversity')} <output>{Math.round(draft.diversity * 100)}%</output></span><input type="range" min="0" max="1" step=".05" value={draft.diversity} onChange={e => setDraft({ ...draft, diversity: Number(e.target.value) })} /></label>
    <label className="field-label">{t('Реже повторять прослушанное', 'Reduce recently played repeats')}<select value={draft.repeat_days} onChange={e => setDraft({ ...draft, repeat_days: Number(e.target.value) })}>{[0, 1, 2, 3, 7].map(days => <option key={days} value={days}>{days ? t(`${days} дн.`, `${days} days`) : t('Без ограничения', 'No restriction')}</option>)}</select></label>
    <p className="muted">{t('В маленькой медиатеке повторы возможны, если новых треков не осталось.', 'A small library can still repeat recordings when fresh tracks run out.')}</p>
    <strong>{t('Предпочитаемые жанры', 'Preferred genres')}</strong><div className="preference-genres">{genres.map(genre => <button key={genre} aria-pressed={draft.genres.includes(genre)} className={draft.genres.includes(genre) ? 'active' : ''} onClick={() => setDraft({ ...draft, genres: draft.genres.includes(genre) ? draft.genres.filter(item => item !== genre) : [...draft.genres, genre] })}>{genre}</button>)}</div>
    <button className="secondary-button" disabled={busy || JSON.stringify(draft) === JSON.stringify(settings.music_taste)} onClick={() => { setBusy(true); setMessage(''); void update('music_taste', draft).then(() => setMessage(t('Вкус сохранён. Запусти новую волну, чтобы применить сразу.', 'Preferences saved. Start a new wave to apply immediately.'))).catch(e => setMessage(String(e))).finally(() => setBusy(false)) }}>{t('Сохранить предпочтения', 'Save preferences')}</button>{message && <p role="status">{message}</p>}
  </div>
}

export function ThemePresets({ settings }: { settings: Settings }) {
  const t = (ru: string, en: string) => settings.language === 'English' ? en : ru
  const queryClient = useQueryClient()
  const [name, setName] = useState('')
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const perform = async (action: 'apply' | 'save' | 'delete' | 'import' | 'export', preset?: ThemePreset, presetName?: string) => {
    setBusy(true); setMessage('')
    try {
      const next = await api.themeAction(action, presetName ?? name.trim(), preset)
      queryClient.setQueryData(['settings'], next)
      setMessage(t('Готово', 'Done'))
    } catch (error) { setMessage(String(error)) }
    finally { setBusy(false) }
  }
  return <div className="settings-card improvement-card"><h3>{t('Пресеты оформления', 'Theme presets')}</h3><div className="preference-genres">{builtInThemes.map(preset => <button key={preset.name} disabled={busy} onClick={() => void perform('apply', preset)}>{preset.name}</button>)}</div>
    <p className="muted">{t('Пресеты сохраняют цвета, размеры и эффекты. Фоновое изображение и файл шрифта выбираются отдельно.', 'Presets include colours, sizes and effects. Choose wallpaper and font files separately.')}</p>
    <form className="inline-form" onSubmit={e => { e.preventDefault(); void perform('save') }}><input value={name} maxLength={64} placeholder={t('Название своей темы', 'Your theme name')} aria-label={t('Название темы', 'Theme name')} onChange={e => setName(e.target.value)} /><button disabled={busy || !name.trim()}>{t('Сохранить текущую', 'Save current theme')}</button></form>
    {savedThemes(settings.theme_presets).map(preset => <div className="saved-theme" key={preset.name}><button disabled={busy} onClick={() => void perform('apply', preset)}>{preset.name}</button><button disabled={busy} aria-label={t('Удалить тему', 'Delete theme') + ': ' + preset.name} onClick={() => void perform('delete', undefined, preset.name)}><X size={16} /></button></div>)}
    <div className="preference-genres"><button disabled={busy} onClick={() => void perform('export')}>{t('Экспорт JSON', 'Export JSON')}</button><button disabled={busy} onClick={() => void perform('import')}>{t('Импорт темы', 'Import theme')}</button></div>{message && <p role="status">{message}</p>}
  </div>
}

export function ProblemReportCard({ english }: { english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const [report, setReport] = useState<ProblemReport | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const create = async () => { setBusy(true); setError(''); try { setReport(await api.problemReport()) } catch (e) { setError(String(e)) } finally { setBusy(false) } }
  return <div className="settings-card"><h3>{t('Помощь с проблемой', 'Troubleshooting')}</h3><p className="muted">{t('Подготовь технический отчёт, проверь его и сохрани файл. Передать его разработчику можно самостоятельно.', 'Prepare a technical report, review it and save the file. You can share it with the developer yourself.')}</p><button disabled={busy} onClick={() => void create()}>{t('Подготовить отчёт', 'Prepare report')}</button>{error && <p role="alert">{error}</p>}
    <Dialog.Root open={!!report} onOpenChange={open => { if (!open) setReport(null) }}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><Dialog.Content className="report-dialog"><div className="queue-heading"><Dialog.Title>{t('Проверь отчёт', 'Review report')}</Dialog.Title><Dialog.Close className="icon-button" aria-label={t('Закрыть', 'Close')}><X size={20} /></Dialog.Close></div><Dialog.Description>{t('Только версия, состояние подключения и аудио. Отчёт не отправляется автоматически.', 'Only version, connection and audio status. Reports are never sent automatically.')}</Dialog.Description><pre>{JSON.stringify(report, null, 2)}</pre><button className="secondary-button" disabled={busy} onClick={() => { if (!report) return; setBusy(true); void api.saveProblemReport(report).catch(e => setError(String(e))).finally(() => setBusy(false)) }}>{t('Сохранить файл', 'Save file')}</button>{error && <p role="alert">{error}</p>}</Dialog.Content></Dialog.Portal></Dialog.Root>
  </div>
}

export function PlaybackError({ error, english, retry }: { error: string; english: boolean; retry?: () => void }) {
  const issue = playbackError(error, english)
  return <small className="error-text playback-issue" role="alert"><span>{issue.message}</span>{issue.action === 'retry' && retry && <button onClick={retry}>{english ? 'Retry' : 'Повторить'}</button>}{issue.action === 'account' && <button onClick={() => { useApp.getState().openAccountSettings() }}>{english ? 'Account' : 'Аккаунт'}</button>}</small>
}
