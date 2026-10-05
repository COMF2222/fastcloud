import { useEffect, useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { X } from 'lucide-react'
import { api } from './api'
import { useApp } from './store'
import type { MusicTaste, Settings } from './types'
import { builtInThemes, savedThemes, themeIsActive, type ThemePreset } from './themePresets'
import { playbackError } from './playbackErrors'

export function TasteSettings({ settings, update }: { settings: Settings; update: (key: string, value: unknown) => Promise<void> }) {
  const t = (ru: string, en: string) => settings.language === 'English' ? en : ru
  const [draft, setDraft] = useState<MusicTaste>(settings.music_taste)
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  useEffect(() => setDraft(settings.music_taste), [settings.music_taste])
  const genres = ['Hip-hop & Rap', 'Trap', 'Phonk', 'R&B', 'Soul', 'Pop', 'Hyperpop', 'Indie', 'Rock', 'Alternative Rock', 'Metal', 'Punk', 'Electronic', 'House', 'Deep House', 'Techno', 'Trance', 'Drum & Bass', 'Dubstep', 'Garage', 'Dance', 'Disco', 'Ambient', 'Lo-fi', 'Chillout', 'Jazz', 'Blues', 'Classical', 'Acoustic', 'Folk', 'Country', 'Reggae']
  return <div className="settings-card improvement-card"><h3>{t('Музыкальный вкус', 'Musical taste')}</h3><p className="muted">{t('Настройки влияют на следующие треки «Моей волны». Лайки и дизлайки продолжают учитываться.', 'These preferences affect upcoming My Wave recommendations. Likes and dislikes still count.')}</p>
    <label className="preference-range"><span>{t('Знакомое → новое', 'Familiar → new')} <output>{Math.round(draft.discovery * 100)}%</output></span><input aria-label={t('Знакомое → новое', 'Familiar → new')} type="range" min="0" max="1" step=".05" value={draft.discovery} onChange={e => setDraft({ ...draft, discovery: Number(e.target.value) })} /></label>
    <label className="preference-range"><span>{t('Разнообразие исполнителей', 'Artist diversity')} <output>{Math.round(draft.diversity * 100)}%</output></span><input aria-label={t('Разнообразие исполнителей', 'Artist diversity')} type="range" min="0" max="1" step=".05" value={draft.diversity} onChange={e => setDraft({ ...draft, diversity: Number(e.target.value) })} /></label>
    <label className="field-label">{t('Реже повторять прослушанное', 'Reduce recently played repeats')}<select value={draft.repeat_days} onChange={e => setDraft({ ...draft, repeat_days: Number(e.target.value) })}>{[0, 1, 2, 3, 7].map(days => <option key={days} value={days}>{days ? t(`${days} дн.`, `${days} days`) : t('Без ограничения', 'No restriction')}</option>)}</select></label>
    <p className="muted">{t('В маленькой медиатеке повторы возможны, если новых треков не осталось.', 'A small library can still repeat recordings when fresh tracks run out.')}</p>
    <div className="preference-heading"><strong>{t('Предпочитаемые жанры', 'Preferred genres')}</strong><small>{draft.genres.length} / 12</small></div><div className="preference-genres">{[...new Set([...genres, ...draft.genres])].map(genre => <button key={genre} disabled={busy || (draft.genres.length >= 12 && !draft.genres.includes(genre))} aria-pressed={draft.genres.includes(genre)} className={draft.genres.includes(genre) ? 'active' : ''} onClick={() => setDraft({ ...draft, genres: draft.genres.includes(genre) ? draft.genres.filter(item => item !== genre) : [...draft.genres, genre] })}>{genre}</button>)}</div>
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
  return <div className="settings-card improvement-card theme-presets"><h3>{t('Пресеты оформления', 'Theme presets')}</h3>
    <p className="muted">{t('Готовые палитры меняют цвета и подложки, сохраняя твой размер текста. Свои темы также сохраняют размеры и эффекты.', 'Ready-made palettes change colours and surfaces while preserving your text size. Custom themes also save sizes and effects.')}</p>
    <div className="theme-preset-grid">{builtInThemes.map(preset => <button className="theme-preset-choice" key={preset.name} aria-pressed={themeIsActive(settings, preset)} disabled={busy} onClick={() => void perform('apply', preset)}><span className="theme-preset-swatches" aria-hidden="true"><i style={{ background: `rgb(${(preset.values.panel_rgb ?? (preset.values.theme === 'Light' ? [249, 249, 251] : [23, 25, 32])).join(' ')})` }} /><i style={{ background: `rgb(${preset.values.accent_rgb!.join(' ')})` }} /></span><span>{preset.name}</span></button>)}</div>
    <details className="theme-preset-custom"><summary>{t('Свои темы и файлы', 'Custom themes and files')}</summary>
    <p className="muted">{t('Фоновое изображение и файл шрифта выбираются отдельно.', 'Choose wallpaper and font files separately.')}</p>
    <form className="inline-form" onSubmit={e => { e.preventDefault(); void perform('save') }}><input value={name} maxLength={64} placeholder={t('Название своей темы', 'Your theme name')} aria-label={t('Название темы', 'Theme name')} onChange={e => setName(e.target.value)} /><button className="preset-action" disabled={busy || !name.trim()}>{t('Сохранить', 'Save')}</button></form>
    {savedThemes(settings.theme_presets).map(preset => <div className="saved-theme" key={preset.name}><button className="preset-action" disabled={busy} onClick={() => void perform('apply', preset)}>{preset.name}</button><button className="icon-button" disabled={busy} aria-label={t('Удалить тему', 'Delete theme') + ': ' + preset.name} onClick={() => void perform('delete', undefined, preset.name)}><X size={16} /></button></div>)}
    <div className="theme-preset-files"><button className="preset-action" disabled={busy} onClick={() => void perform('export')}>{t('Экспорт JSON', 'Export JSON')}</button><button className="preset-action" disabled={busy} onClick={() => void perform('import')}>{t('Импорт темы', 'Import theme')}</button></div></details>{message && <p role="status">{message}</p>}
  </div>
}

export function PlaybackError({ error, english, retry }: { error: string; english: boolean; retry?: () => void }) {
  const issue = playbackError(error, english)
  return <small className="error-text playback-issue" role="alert"><span>{issue.message}</span>{issue.action === 'retry' && retry && <button onClick={retry}>{english ? 'Retry' : 'Повторить'}</button>}{issue.action === 'account' && <button onClick={() => { useApp.getState().openAccountSettings() }}>{english ? 'Account' : 'Аккаунт'}</button>}</small>
}
