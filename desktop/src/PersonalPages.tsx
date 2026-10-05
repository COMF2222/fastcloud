import { useEffect, useMemo, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import * as Dialog from '@radix-ui/react-dialog'
import { ArrowDown, ArrowUp, ChartColumn, Clock3, Folder, Pin, Play, Plus, Timer, X } from 'lucide-react'
import { api } from './api'
import { useApp } from './store'
import { musicGenres } from './GenreCarousel'
import { artist, libraryCollections, type PlayerState, type PlaylistFolder, type Settings, type SmartPlaylist } from './types'

const syncLabel = (status: string | undefined, english: boolean) => status === 'synced' ? english ? 'Synced with your account' : 'Синхронизировано с аккаунтом' : status === 'demo' ? english ? 'Interface demo' : 'Предпросмотр интерфейса' : english ? 'Local copy · sync when connected' : 'Локальная копия · синхронизация при подключении'
const listeningTime = (ms: number, english: boolean) => {
  const minutes = Math.floor(ms / 60000)
  return minutes >= 60 ? `${Math.floor(minutes / 60)} ${english ? 'h' : 'ч'} ${minutes % 60} ${english ? 'min' : 'мин'}` : `${minutes} ${english ? 'min' : 'мин'}`
}

export function StatisticsPage({ english }: { english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const { data, error } = useQuery({ queryKey: ['listening-statistics'], queryFn: api.listeningStatistics, refetchInterval: 5000 })
  const [days, setDays] = useState(7)
  const [message, setMessage] = useState('')
  const client = useQueryClient()
  const chart = Array.from({ length: days }, (_, index) => {
    const date = new Date(); date.setUTCDate(date.getUTCDate() - days + index + 1)
    const day = date.toISOString().slice(0, 10)
    return { day, ms: data?.daily.find(row => row.day === day)?.ms || 0 }
  })
  const total = chart.reduce((sum, row) => sum + row.ms, 0)
  const maximum = Math.max(60000, ...chart.map(row => row.ms))
  const artists = useMemo(() => {
    const groups = new Map<string, number>()
    for (const row of data?.tracks || []) groups.set(row.artist, (groups.get(row.artist) || 0) + row.ms)
    return [...groups].sort((a, b) => b[1] - a[1]).slice(0, 5)
  }, [data?.tracks])
  const genres = useMemo(() => {
    const groups = new Map<string, number>()
    for (const row of data?.tracks || []) if (row.genre) groups.set(row.genre, (groups.get(row.genre) || 0) + row.ms)
    return [...groups].sort((a, b) => b[1] - a[1]).slice(0, 5)
  }, [data?.tracks])
  const play = async (id: number) => {
    try { const track = await api.track(id); if (track.status !== 'ready') throw new Error(t('Трек пока недоступен.', 'Track is not available yet.')); await api.play([track.data], 0); await client.invalidateQueries({ queryKey: ['player'] }) }
    catch (error) { setMessage(String(error)) }
  }
  return <div className="page-content personal-page"><div className="section-title"><div><h2>{t('Твоя статистика', 'Your listening stats')}</h2><p>{t('Считается время звучащей музыки. Паузы, загрузка и перемотка не добавляют минуты.', 'Only moving audio counts. Pauses, loading and seeking do not add minutes.')}</p></div><ChartColumn size={28} /></div>
    <p className="muted">{syncLabel(data?.syncStatus, english)}</p>
    {(error || message) && <p role="alert" className="error-text">{message || String(error)}</p>}
    <div className="listening-summary"><div><strong>{listeningTime(data?.totals.ms || 0, english)}</strong><small>{t('за всё время', 'all time')}</small></div><div><strong>{data?.totals.plays || 0}</strong><small>{t('прослушиваний', 'listens')}</small></div><div><strong>{data?.tracks.length || 0}</strong><small>{t('разных треков', 'unique tracks')}</small></div></div>
    <section className="settings-card"><div className="personal-section-heading"><h3>{t('Активность', 'Listening activity')} · {listeningTime(total, english)}</h3><div className="preference-genres">{[7, 30].map(value => <button key={value} aria-pressed={days === value} onClick={() => setDays(value)}>{value} {t('дней', 'days')}</button>)}</div></div><div className="listening-chart" role="img" aria-label={t(`Прослушивание за ${days} дней, ${listeningTime(total, false)}`, `Listening over ${days} days, ${listeningTime(total, true)}`)}>{chart.map(row => <div key={row.day} title={`${row.day}: ${listeningTime(row.ms, english)}`}><span style={{ height: `${Math.max(row.ms ? 3 : 0, row.ms / maximum * 100)}%` }} /><small>{days === 7 ? new Date(row.day + 'T12:00:00Z').toLocaleDateString(english ? 'en-GB' : 'ru-RU', { weekday: 'short', timeZone: 'UTC' }) : row.day.slice(8)}</small></div>)}</div><p className="muted">{t('Дни отображаются по UTC. Прослушивание засчитывается после 30 секунд или половины короткого трека.', 'Days use UTC. A listen counts after 30 seconds or half of a short track.')}</p></section>
    {!data?.tracks.length && <div className="empty"><Clock3 size={30} /><h3>{t('Начни слушать', 'Start listening')}</h3><p>{t('Статистика появится здесь после прослушивания. Прошлые минуты задним числом не добавляются.', 'Your statistics appear as you listen. Earlier listening time is not reconstructed.')}</p></div>}
    <div className="personal-columns"><section className="settings-card"><h3>{t('Любимые исполнители · всё время', 'Top artists · all time')}</h3>{artists.map(([name, ms]) => <div className="listening-ranking" key={name}><strong>{name}</strong><span>{listeningTime(ms, english)}</span></div>)}</section><section className="settings-card"><h3>{t('Жанры · всё время', 'Genres · all time')}</h3>{genres.map(([name, ms]) => <div className="listening-ranking" key={name}><strong>{name}</strong><span>{listeningTime(ms, english)}</span></div>)}</section></div>
    {!!data?.tracks.length && <section className="settings-card"><h3>{t('Самые прослушанные треки · всё время', 'Top tracks · all time')}</h3>{data.tracks.slice(0, 10).map((row, index) => <button className="listening-track" key={row.trackId} onClick={() => void play(row.trackId)}><span>{index + 1}</span><div><strong>{row.title}</strong><small>{row.artist}</small></div><span>{listeningTime(row.ms, english)}</span><Play size={15} /></button>)}</section>}
  </div>
}

export function CollectionsPage({ english }: { english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const client = useQueryClient()
  const { data } = useQuery({ queryKey: ['personal-collections'], queryFn: api.personalCollections, refetchInterval: 10000 })
  const { data: mine } = useQuery({ queryKey: ['playlists', 'mine'], queryFn: () => api.playlists('mine'), refetchInterval: query => query.state.data?.status === 'loading' ? 500 : false })
  const { data: liked } = useQuery({ queryKey: ['playlists', 'liked'], queryFn: () => api.playlists('liked'), refetchInterval: query => query.state.data?.status === 'loading' ? 500 : false })
  const collection = libraryCollections([mine, liked], false)
  const playlists = collection.status === 'ready' ? collection.data : []
  const folders = Object.entries(data?.folders || {}).sort((a, b) => Number(b[1].pinned) - Number(a[1].pinned) || a[1].order - b[1].order || a[0].localeCompare(b[0]))
  const [folderId, setFolderId] = useState('')
  const [name, setName] = useState('')
  const [query, setQuery] = useState('')
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [rule, setRule] = useState<SmartPlaylist>({ name: '', genre: '', addedDays: 0, unplayedDays: 0, limit: 50 })
  const [smartId, setSmartId] = useState('')
  const folder = data?.folders[folderId]
  useEffect(() => { setName(folder?.name || '') }, [folder?.name])
  const { data: smartTracks } = useQuery({ queryKey: ['smart-playlist', smartId], queryFn: () => api.smartPlaylistTracks(smartId), enabled: !!smartId, refetchInterval: query => query.state.data?.status === 'loading' ? 500 : 15000 })
  const change = async (work: () => Promise<unknown>) => { setBusy(true); setError(''); try { await work(); await client.invalidateQueries({ queryKey: ['personal-collections'] }); await client.invalidateQueries({ queryKey: ['smart-playlist'] }) } catch (error) { setError(String(error)) } finally { setBusy(false) } }
  const saveFolder = () => change(async () => { const id = folderId || crypto.randomUUID(); await api.updatePersonalCollection('folders', id, { name: name.trim(), playlistIds: folder?.playlistIds || [], pinned: folder?.pinned || false, order: folder?.order ?? folders.length }); setFolderId(id) })
  const assign = (playlistId: number, target: string) => change(async () => {
    for (const [id, folder] of folders) {
      const ids = folder.playlistIds.filter(id => id !== playlistId)
      if (id === target) ids.push(playlistId)
      if (JSON.stringify(ids) !== JSON.stringify(folder.playlistIds)) await api.updatePersonalCollection('folders', id, { ...folder, playlistIds: ids })
    }
  })
  const move = (index: number, direction: number) => change(async () => {
    const swapped = [...folders]; [swapped[index], swapped[index + direction]] = [swapped[index + direction], swapped[index]]
    for (const [order, [id, folder]] of swapped.entries()) if (folder.order !== order) await api.updatePersonalCollection('folders', id, { ...folder, order })
  })
  const visible = playlists.filter(item => (!folder || folder.playlistIds.includes(item.id)) && `${item.title} ${item.user?.username || ''}`.toLocaleLowerCase().includes(query.toLocaleLowerCase()))
  return <div className="page-content personal-page"><div className="section-title"><div><h2>{t('Папки и умные плейлисты', 'Folders & smart playlists')}</h2><p>{t('Наведи порядок в своих и сохранённых плейлистах. Папки не меняют оригиналы в SoundCloud.', 'Organise your own and saved playlists. Folders leave SoundCloud originals intact.')}</p></div><Folder size={26} /></div><p className="muted">{syncLabel(data?.syncStatus, english)}</p>{error && <p role="alert" className="error-text">{error}</p>}
    <section className="settings-card"><h3>{t('Папки плейлистов', 'Playlist folders')}</h3><div className="folder-tabs"><button aria-pressed={!folderId} onClick={() => { setFolderId(''); setName('') }}>{t('Все плейлисты', 'All playlists')}</button>{folders.map(([id, folder], index) => <div className="folder-tab" key={id}><button aria-pressed={folderId === id} onClick={() => setFolderId(id)}>{folder.pinned && <Pin size={12} />}{folder.name}</button><button className="icon-button" disabled={busy || index === 0 || (index > 0 && folders[index - 1][1].pinned !== folder.pinned)} aria-label={t('Поднять папку', 'Move folder up') + ': ' + folder.name} onClick={() => void move(index, -1)}><ArrowUp size={12} /></button><button className="icon-button" disabled={busy || index === folders.length - 1 || (index < folders.length - 1 && folders[index + 1][1].pinned !== folder.pinned)} aria-label={t('Опустить папку', 'Move folder down') + ': ' + folder.name} onClick={() => void move(index, 1)}><ArrowDown size={12} /></button></div>)}</div>
      <form className="inline-form personal-form" onSubmit={event => { event.preventDefault(); void saveFolder() }}><input maxLength={128} value={name} aria-label={t('Название папки', 'Folder name')} placeholder={folder ? t('Новое название папки', 'Rename folder') : t('Название новой папки', 'New folder name')} onChange={event => setName(event.target.value)} /><button className="secondary-button" disabled={busy || !name.trim()}>{folder ? t('Переименовать', 'Rename') : t('Создать папку', 'Create folder')}</button>{folder && <><button type="button" className="secondary-button" disabled={busy} onClick={() => void change(() => api.updatePersonalCollection('folders', folderId, { ...folder, pinned: !folder.pinned }))}>{folder.pinned ? t('Открепить', 'Unpin') : t('Закрепить', 'Pin')}</button><button type="button" className="secondary-button" disabled={busy} onClick={() => { if (window.confirm(t('Удалить папку? Плейлисты останутся.', 'Delete this folder? Playlists will remain.'))) void change(async () => { await api.updatePersonalCollection('folders', folderId, null); setFolderId('') }) }}>{t('Удалить папку', 'Delete folder')}</button></>}</form>
      <input className="personal-search" value={query} onChange={event => setQuery(event.target.value)} aria-label={t('Поиск в папке', 'Search folder')} placeholder={t('Плейлист или автор…', 'Playlist or creator…')} />
      {collection.status === 'loading' ? <p>{t('Загружаем плейлисты…', 'Loading playlists…')}</p> : collection.status === 'failed' ? <p role="alert" className="error-text">{collection.data}</p> : visible.length ? <div className="folder-playlists">{visible.map(item => <div key={item.id}><button onClick={() => useApp.getState().openPlaylist(item.id, item.title)}><strong>{item.title}</strong><small>{item.user?.username || '—'}</small></button><select disabled={busy} aria-label={t('Папка плейлиста', 'Playlist folder') + ': ' + item.title} value={folders.find(([,folder]) => folder.playlistIds.includes(item.id))?.[0] || ''} onChange={event => void assign(item.id, event.target.value)}><option value="">{t('Без папки', 'No folder')}</option>{folders.map(([id, folder]) => <option key={id} value={id}>{folder.name}</option>)}</select></div>)}</div> : <p className="muted">{t('Здесь пока нет плейлистов. Выбери «Все плейлисты», чтобы распределить их по папкам.', 'No playlists here yet. Choose All playlists to organise them into folders.')}</p>}
    </section>
    <section className="settings-card"><h3>{t('Умные плейлисты', 'Smart playlists')}</h3><p>{t('Правила автоматически выбирают треки из твоих лайков при открытии подборки.', 'Rules automatically select tracks from your likes whenever you open the collection.')}</p><form className="smart-rule-form" onSubmit={event => { event.preventDefault(); void change(async () => { const id = smartId || crypto.randomUUID(); await api.updatePersonalCollection('smartPlaylists', id, { ...rule, name: rule.name.trim() }); setSmartId(id) }) }}>
      <label className="field-label">{t('Название подборки', 'Collection name')}<input maxLength={128} value={rule.name} onChange={event => setRule({ ...rule, name: event.target.value })} /></label><label className="field-label">{t('Жанр', 'Genre')}<select value={rule.genre} onChange={event => setRule({ ...rule, genre: event.target.value })}><option value="">{t('Любой жанр', 'Any genre')}</option>{musicGenres.map(genre => <option key={genre} value={genre}>{genre}</option>)}</select></label>
      <label className="field-label">{t('Недавние лайки', 'Recent likes')}<select value={rule.addedDays} onChange={event => setRule({ ...rule, addedDays: Number(event.target.value) })}>{[0, 7, 30, 90].map(days => <option key={days} value={days}>{days ? t(`За ${days} дней`, `Last ${days} days`) : t('Любая дата', 'Any date')}</option>)}</select></label><label className="field-label">{t('Давно не слушал', 'Not played recently')}<select value={rule.unplayedDays} onChange={event => setRule({ ...rule, unplayedDays: Number(event.target.value) })}>{[0, 7, 30, 90].map(days => <option key={days} value={days}>{days ? t(`Не слушал ${days} дней`, `Not played in ${days} days`) : t('Без ограничения', 'No restriction')}</option>)}</select></label>
      <label className="field-label">{t('Максимум треков', 'Maximum tracks')}<select value={rule.limit} onChange={event => setRule({ ...rule, limit: Number(event.target.value) })}>{[25, 50, 100, 250, 500].map(limit => <option key={limit} value={limit}>{limit}</option>)}</select></label><button className="secondary-button" disabled={busy || !rule.name.trim()}>{smartId ? t('Сохранить правило', 'Save rule') : t('Создать подборку', 'Create collection')}</button></form><p className="muted">{t('Дата новых лайков учитывается с момента использования Fastcloud; дата старых лайков может быть неизвестна.', 'New like dates are tracked while using Fastcloud; older like dates may be unknown.')}</p>
      <div className="smart-playlist-list">{Object.entries(data?.smartPlaylists || {}).map(([id, saved]) => <div key={id}><button aria-pressed={smartId === id} onClick={() => { setSmartId(id); setRule(saved) }}><strong>{saved.name}</strong><small>{saved.genre || t('Все жанры', 'All genres')} · {saved.limit} {t('треков максимум', 'tracks maximum')}</small></button><button className="icon-button" disabled={busy} aria-label={t('Удалить подборку', 'Delete collection') + ': ' + saved.name} onClick={() => void change(async () => { await api.updatePersonalCollection('smartPlaylists', id, null); if (smartId === id) { setSmartId(''); setRule({ name: '', genre: '', addedDays: 0, unplayedDays: 0, limit: 50 }) } })}><X size={16} /></button></div>)}</div>
      {smartId && <button className="text-button" onClick={() => { setSmartId(''); setRule({ name: '', genre: '', addedDays: 0, unplayedDays: 0, limit: 50 }) }}><Plus size={14} />{t('Ещё одна подборка', 'Another collection')}</button>}
      {smartId && (smartTracks?.status === 'loading' ? <p>{t('Обновляем подборку…', 'Updating collection…')}</p> : smartTracks?.status === 'ready' ? <><div className="personal-section-heading"><strong>{smartTracks.data.length} {t('треков по правилу', 'matching tracks')}</strong><button className="secondary-button" disabled={!smartTracks.data.length} onClick={() => void change(async () => { await api.play(smartTracks.data, 0); await client.invalidateQueries({ queryKey: ['player'] }) })}><Play size={14} />{t('Слушать всё', 'Play all')}</button></div>{smartTracks.data.slice(0, 50).map(track => <button className="listening-track" key={track.id} onClick={() => void change(async () => { await api.play(smartTracks.data, smartTracks.data.findIndex(item => item.id === track.id)); await client.invalidateQueries({ queryKey: ['player'] }) })}><div><strong>{track.title}</strong><small>{artist(track)}</small></div><Play size={14} /></button>)}</> : smartTracks?.status === 'failed' ? <p role="alert" className="error-text">{smartTracks.data}</p> : <p>{t('Подключи SoundCloud для подборки по лайкам.', 'Connect SoundCloud for collections based on your likes.')}</p>)}
    </section>
  </div>
}

export function BackupSettings({ english }: { english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const client = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [message, setMessage] = useState('')
  const { data } = useQuery({ queryKey: ['personal-collections'], queryFn: api.personalCollections, refetchInterval: 10000 })
  const run = async (action: 'export' | 'import' | 'sync') => {
    setBusy(true); setMessage('')
    try {
      if (action === 'sync') await api.syncPersonalData()
      else client.setQueryData(['settings'], await api.settingsBackup(action))
      await client.invalidateQueries({ queryKey: ['settings'] }); await client.invalidateQueries({ queryKey: ['personal-collections'] })
      setMessage(t('Готово', 'Done'))
    } catch (error) { setMessage(String(error)) } finally { setBusy(false) }
  }
  return <section className="settings-card"><h3>{t('Данные аккаунта и резервная копия', 'Account data & backup')}</h3><p>{t('Оформление, музыкальный вкус, свои пресеты, закрепления, папки и правила подборок хранятся в аккаунте на сервере. Локальная копия работает без сети.', 'Appearance, musical taste, custom presets, pins, folders and collection rules are stored with your account on the server. The local copy works offline.')}</p><p className="muted">{syncLabel(data?.syncStatus, english)}</p><div className="inline-form"><button className="secondary-button" disabled={busy} onClick={() => void run('sync')}>{t('Синхронизировать', 'Sync now')}</button><button className="secondary-button" disabled={busy} onClick={() => void run('export')}>{t('Сохранить копию JSON', 'Export JSON backup')}</button><button className="secondary-button" disabled={busy} onClick={() => { if (window.confirm(t('Импортировать настройки из файла? Настройки оформления будут заменены, папки и подборки добавлены. Данные входа останутся.', 'Import settings from a file? Appearance settings will be replaced and collections merged. Sign-in data will stay.'))) void run('import') }}>{t('Восстановить из файла', 'Restore from file')}</button></div><p className="muted">{t('В копию не входят токены, история прослушивания, локальные пути к обоям и шрифтам, скачанные треки и настройки устройства.', 'The backup excludes tokens, listening history, local wallpaper/font paths, downloads and device settings.')}</p>{message && <p role="status">{message}</p>}</section>
}

export function PlaybackOptions({ settings, update }: { settings: Settings; update: (key: string, value: unknown) => Promise<void> }) {
  const t = (ru: string, en: string) => settings.language === 'English' ? en : ru
  return <section className="settings-card"><h3>{t('Громкость и переходы', 'Loudness & transitions')}</h3><label className="setting-row"><span><strong>{t('Выравнивание громкости', 'Volume normalization')}</strong><small>{t('Уменьшает разницу между тихими и громкими записями, без изменения файлов. Оценка по началу трека; усиление ограничено.', 'Reduces differences between quiet and loud recordings without changing files. Estimated from the opening, with limited amplification.')}</small></span><input type="checkbox" checked={settings.normalization} onChange={event => void update('normalization', event.target.checked)} /></label><label className="field-label">{t('Плавный переход', 'Crossfade')}<select value={settings.crossfade_ms} onChange={event => void update('crossfade_ms', Number(event.target.value))}>{[0, 2000, 4000, 6000, 8000].map(ms => <option key={ms} value={ms}>{ms ? t(`${ms / 1000} секунд`, `${ms / 1000} seconds`) : t('Выключен', 'Off')}</option>)}</select></label><label className="setting-row"><span><strong>{t('Без пауз между треками', 'Gapless playback')}</strong><small>{t('Заранее готовит следующий трек. Для альбомов выключи плавный переход, чтобы не смешивать записи.', 'Prepares the next track ahead of time. Disable crossfade for albums to avoid mixing recordings.')}</small></span><input type="checkbox" checked={settings.gapless} onChange={event => void update('gapless', event.target.checked)} /></label><p className="muted">{t('Переход начинается только когда следующий трек готов. При сбое сети текущий трек доигрывает обычным способом.', 'Transitions start only when the next track is ready. If preparation fails, the current track ends normally.')}</p></section>
}

export function SleepTimerButton({ state, english }: { state?: PlayerState; english: boolean }) {
  const t = (ru: string, en: string) => english ? en : ru
  const [open, setOpen] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const client = useQueryClient()
  const active = state?.sleepRemainingMs != null || state?.sleepAfterTrack
  const set = async (seconds: number | null, afterTrack = false) => { setBusy(true); setError(''); try { await api.sleepTimer(seconds, afterTrack); await client.invalidateQueries({ queryKey: ['player'] }); setOpen(false) } catch (error) { setError(String(error)) } finally { setBusy(false) } }
  return <><button className={`icon-button sleep-timer-trigger ${active ? 'on' : ''}`} aria-label={t('Таймер сна', 'Sleep timer')} title={active ? state?.sleepAfterTrack ? t('Остановить после трека', 'Stop after this track') : `${t('Таймер сна', 'Sleep timer')}: ${Math.ceil((state?.sleepRemainingMs || 0) / 60000)} ${t('мин', 'min')}` : t('Таймер сна', 'Sleep timer')} onClick={() => setOpen(true)}><Timer size={17} /></button><Dialog.Root open={open} onOpenChange={setOpen}><Dialog.Portal><Dialog.Overlay className="dialog-overlay" /><Dialog.Content className="sleep-dialog"><div className="personal-section-heading"><Dialog.Title>{t('Таймер сна', 'Sleep timer')}</Dialog.Title><Dialog.Close className="icon-button" aria-label={t('Закрыть таймер', 'Close sleep timer')}><X size={18} /></Dialog.Close></div><Dialog.Description>{t('Музыка постепенно стихнет в последние 10 секунд и остановится. Настройка действует до завершения таймера или закрытия приложения.', 'Music fades over the last 10 seconds, then pauses. The timer lasts until it finishes or the app closes.')}</Dialog.Description><div className="sleep-presets">{[15, 30, 45, 60, 90, 120].map(minutes => <button className="secondary-button" disabled={busy} key={minutes} onClick={() => void set(minutes * 60)}>{minutes} {t('мин', 'min')}</button>)}</div><button className="secondary-button" disabled={busy || state?.current == null} onClick={() => void set(null, true)}>{t('После текущего трека', 'After current track')}</button>{active && <><p role="status">{state?.sleepAfterTrack ? t('Остановка после текущего трека', 'Pausing after the current track') : `${t('Осталось', 'Remaining')}: ${Math.ceil((state?.sleepRemainingMs || 0) / 60000)} ${t('мин', 'min')}`}</p><button className="secondary-button" disabled={busy} onClick={() => void set(null)}>{t('Отменить таймер', 'Cancel timer')}</button></>}{error && <p role="alert" className="error-text">{error}</p>}</Dialog.Content></Dialog.Portal></Dialog.Root></>
}
