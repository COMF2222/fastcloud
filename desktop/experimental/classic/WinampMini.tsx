import { useEffect, useRef, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { api } from './api'
import { artist, type Settings } from './types'

const sheets = ['main', 'titlebar', 'cbuttons', 'shufrep', 'posbar', 'volume', 'eqmain', 'pledit'] as const
type Sheet = typeof sheets[number]

export function WinampMini({ settings }: { settings: Settings }) {
  const queryClient = useQueryClient()
  const english = settings.language === 'English'
  const t = (ru: string, en: string) => english ? en : ru
  const [showPresets, setShowPresets] = useState(false)
  const [dragRows, setDragRows] = useState<number | null>(null)
  const resizeStart = useRef<{ y: number; rows: number } | null>(null)
  const { data: player, refetch } = useQuery({ queryKey: ['player'], queryFn: api.player, refetchInterval: 500, retry: false })
  const { data: custom = {}, error: skinError } = useQuery({ queryKey: ['skin-images', settings.winamp_skin], queryFn: api.skinImages, retry: false })
  const { data: visualiser } = useQuery({ queryKey: ['visualiser', settings.visualiser], queryFn: api.visualiserFrame, refetchInterval: settings.visualiser === 'Off' ? false : 60, retry: false })
  const image = (sheet: Sheet) => custom[sheet] || `/skins/fastcloud/${sheet}.bmp`
  const bg = (sheet: Sheet, x = 0, y = 0): React.CSSProperties => ({ backgroundImage: `url("${image(sheet)}")`, backgroundPosition: `${-x}px ${-y}px`, backgroundRepeat: 'no-repeat' })
  const track = player?.current == null ? null : player.queue[player.current]
  const length = Math.max(1, player?.durationMs || track?.full_duration_ms || track?.duration || 1)
  const position = Math.min(length, player?.positionMs || 0)
  const clock = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, '0')}`
  const change = async (key: string, value: unknown) => { await api.setSetting(key, value); await queryClient.invalidateQueries({ queryKey: ['settings'] }) }
  const command = async (action: string, value?: number, index?: number) => { await api.transport(action, value, index); await refetch() }
  const action = (label: string, x: number, y: number, w: number, h: number, sheet: Sheet, sx: number, sy: number, click: () => void, pressed = false) => <button type="button" className="wa-sprite" aria-label={label} title={label} style={{ left: x, top: y, width: w, height: h, ...bg(sheet, sx, sy), filter: pressed ? 'brightness(1.22)' : undefined }} onClick={click} />
  const scale = Math.max(1, Math.min(4, settings.winamp_scale || 2))
  const mainHeight = settings.winamp_shade ? 14 : 116
  const eqHeight = settings.winamp_eq_window ? settings.winamp_eq_shade ? 14 : 116 : 0
  const rows = Math.max(4, Math.min(64, dragRows ?? (settings.winamp_pl_rows || 8)))
  useEffect(() => { if (resizeStart.current == null) setDragRows(null) }, [settings.winamp_pl_rows])
  const listHeight = settings.winamp_pl_window ? settings.winamp_pl_shade ? 14 : 58 + rows * 13 : 0
  const eqPoints = settings.eq_gains_db.map((gain, index) => `${index * 12 + 2},${Math.max(1, Math.min(17, 9 - gain * 8 / 12))}`).join(' ')
  const totalHeight = mainHeight + eqHeight + listHeight
  const currentIndex = player?.current ?? -1
  const [red, green, blue] = settings.accent_rgb.map(value => value / 255)
  const top = Math.max(red, green, blue)
  const difference = top - Math.min(red, green, blue)
  const hue = difference === 0 ? 18 : top === red ? ((green - blue) / difference) * 60 : top === green ? ((blue - red) / difference + 2) * 60 : ((red - green) / difference + 4) * 60
  const hueShift = ((hue + 360) % 360) - 18
  const eqPresets = [
    { name: t('Ровно', 'Flat'), gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { name: t('Бас', 'Bass'), gains: [5, 4, 3, 1, 0, 0, 0, 0, 0, 0] },
    { name: t('Вокал', 'Vocals'), gains: [-2, -1, 0, 1, 3, 4, 3, 1, 0, -1] },
    { name: t('Яркость', 'Bright'), gains: [-1, -1, 0, 0, 0, 1, 2, 3, 4, 3] },
  ]
  const finishResize = async (element: HTMLElement, pointerId: number) => {
    if (!resizeStart.current) return
    resizeStart.current = null
    if (element.hasPointerCapture(pointerId)) element.releasePointerCapture(pointerId)
    if (dragRows != null && dragRows !== settings.winamp_pl_rows) await change('winamp_pl_rows', dragRows)
    setDragRows(null)
  }
  return <div className={`wa-outer ${settings.winamp_skin ? '' : 'wa-default-skin'}`} style={{ width: 275 * scale, height: totalHeight * scale, '--wa-hue-shift': `${hueShift}deg`, '--wa-saturation': top ? Math.min(1, difference / top) : 0 } as React.CSSProperties}><div className="wa-stack" style={{ width: 275, height: totalHeight, transform: `scale(${scale})` }}>
    <section className="wa-main" style={{ height: mainHeight, ...bg('main') }} aria-label={t("Классический мини-плеер", "Classic mini player")}>
      <div className="wa-title" style={bg('titlebar', 27, settings.winamp_shade ? 29 : 0)} />
      <div className="wa-drag" onMouseDown={event => { if (event.button === 0 && !api.preview) void getCurrentWindow().startDragging() }} />
      {action(t('Развернуть приложение', 'Restore application'), 6, 3, 9, 9, 'titlebar', 0, 0, () => void change('winamp_window', false))}
      {action(t('Свернуть окно', 'Minimize window'), 244, 3, 9, 9, 'titlebar', 9, 0, () => { if (!api.preview) void getCurrentWindow().minimize() })}
      {action(settings.winamp_shade ? t('Развернуть мини-плеер', 'Expand mini player') : t('Свернуть до заголовка', 'Collapse to title bar'), 254, 3, 9, 9, 'titlebar', 0, settings.winamp_shade ? 27 : 18, () => void change('winamp_shade', !settings.winamp_shade))}
      {action(t('Закрыть мини-плеер', 'Close mini player'), 264, 3, 9, 9, 'titlebar', 18, 0, () => void change('winamp_window', false))}
      {settings.winamp_shade ? <><div className="wa-shade-name" title={track?.title || ''}>{track?.title || 'FASTCLOUD'}</div><div className="wa-shade-time">{clock(position)}</div>{[[t('Предыдущий', 'Previous'), 169, 'previous'], [t('Воспроизвести / пауза', 'Play / pause'), 177, 'toggle'], [t('Стоп', 'Stop'), 195, 'stop'], [t('Следующий', 'Next'), 204, 'next']].map(([label, x, op]) => <button key={String(op)} className="wa-shade-control" style={{ left: Number(x) }} aria-label={String(label)} onClick={() => void command(String(op))} />)}<input className="wa-shade-seek" aria-label={t("Позиция трека", "Track position")} type="range" min={0} max={length} value={position} onChange={event => void command('seek', Number(event.target.value))} /></> : <>
        <div className="wa-time">{clock(position)}</div><div className="wa-name" title={track ? `${artist(track)} — ${track.title}` : ''}>{track ? `${artist(track)} — ${track.title}` : 'FASTCLOUD'}</div>
        <div className="wa-audio-meta">{player?.loading ? 'LOAD' : player?.error ? 'ERROR' : player?.isPlaying ? 'PLAY' : 'STOP'}</div>
        <button className="wa-visualiser" title={t("Сменить режим визуализатора", "Change visualizer mode")} aria-label={`${t("Визуализатор", "Visualizer")}: ${settings.visualiser}`} onClick={() => void change('visualiser', settings.visualiser === 'Spectrum' ? 'Scope' : settings.visualiser === 'Scope' ? 'Off' : 'Spectrum')}><svg width="76" height="16" viewBox="0 0 76 16" aria-hidden="true">{settings.visualiser === 'Spectrum' && visualiser?.bars.map((height, index) => <g key={index}><rect x={index * 4} y={16 - height} width="3" height={height} fill="#ff9c66" />{visualiser.peaks[index] != null && <rect x={index * 4} y={16 - visualiser.peaks[index]!} width="3" height="1" fill="#ffd5b9" />}</g>)}{settings.visualiser === 'Scope' && <polyline points={(visualiser?.scope || []).map((row, index) => `${index},${row}`).join(' ')} fill="none" stroke="#ff9c66" strokeWidth="1" />}</svg></button>
        <input className="wa-volume" type="range" min={0} max={100} value={Math.round((player?.volume || 0) * 100)} aria-label={t("Громкость", "Volume")} onChange={event => void command('volume', Number(event.target.value) / 100)} />
        {action(t('Эквалайзер', 'Equalizer'), 219, 58, 23, 12, 'shufrep', 0, settings.winamp_eq_window ? 73 : 61, () => void change('winamp_eq_window', !settings.winamp_eq_window))}
        {action(t('Очередь', 'Queue'), 242, 58, 23, 12, 'shufrep', 23, settings.winamp_pl_window ? 73 : 61, () => void change('winamp_pl_window', !settings.winamp_pl_window))}
        <div className="wa-seek-art" style={bg('posbar')} /><input className="wa-seek" type="range" min={0} max={length} value={position} aria-label={t("Позиция трека", "Track position")} onChange={event => void command('seek', Number(event.target.value))} />
        {action(t('Предыдущий трек', 'Previous track'), 16, 88, 23, 18, 'cbuttons', 0, 0, () => void command('previous'))}
        {action(t('Воспроизвести', 'Play'), 39, 88, 23, 18, 'cbuttons', 23, 0, () => void command('toggle'))}
        {action(t('Пауза', 'Pause'), 62, 88, 23, 18, 'cbuttons', 46, 0, () => void command('toggle'))}
        {action(t('Стоп', 'Stop'), 85, 88, 23, 18, 'cbuttons', 69, 0, () => void command('stop'))}
        {action(t('Следующий трек', 'Next track'), 108, 88, 22, 18, 'cbuttons', 92, 0, () => void command('next'))}
        {action(t('Открыть очередь', 'Open queue'), 136, 89, 22, 16, 'cbuttons', 114, 0, () => void change('winamp_pl_window', true))}
        {action(t('Перемешать', 'Shuffle'), 164, 89, 47, 15, 'shufrep', 28, player?.shuffle ? 30 : 0, () => void command('shuffle'))}
        {action(`${t('Повтор', 'Repeat')}: ${player?.repeat || 'Off'}`, 210, 89, 28, 15, 'shufrep', 0, player?.repeat === 'Off' ? 0 : 30, () => void command('repeat'))}
      </>}
    </section>
    {settings.winamp_eq_window && <section className={`wa-eq ${settings.winamp_eq_shade ? 'shade' : ''}`} style={{ top: mainHeight, height: eqHeight, ...bg('eqmain') }} aria-label={t("Эквалайзер", "Equalizer")}>
      <div className="wa-eq-heading">EQUALIZER<div><button aria-label={settings.winamp_eq_shade ? t('Развернуть эквалайзер', 'Expand equalizer') : t('Свернуть эквалайзер', 'Collapse equalizer')} onClick={() => void change('winamp_eq_shade', !settings.winamp_eq_shade)}>▴</button><button aria-label={t("Закрыть эквалайзер", "Close equalizer")} onClick={() => void change('winamp_eq_window', false)}>×</button></div></div>
      {!settings.winamp_eq_shade && <><label className="wa-eq-on"><input type="checkbox" checked={settings.eq_enabled} onChange={event => void change('eq_enabled', event.target.checked)} /> ON</label><label className="wa-eq-auto"><input type="checkbox" checked={settings.eq_auto} onChange={event => void change('eq_auto', event.target.checked)} /> AUTO</label><button className="wa-eq-presets" aria-expanded={showPresets} aria-label={t('Пресеты эквалайзера', 'Equalizer presets')} onClick={() => setShowPresets(value => !value)}>PRESETS</button>{showPresets && <div className="wa-eq-menu" role="menu">{eqPresets.map(preset => <button key={preset.name} role="menuitem" onClick={() => { void change('eq_enabled', true).then(() => change('eq_gains_db', preset.gains)); setShowPresets(false) }}>{preset.name}</button>)}<button role="menuitem" disabled={player?.current == null} onClick={() => { void api.eqPreset(); setShowPresets(false) }}>{t('Сохранить для трека', 'Save for track')}</button><button role="menuitem" disabled={player?.current == null} onClick={() => { void api.eqPreset(true); setShowPresets(false) }}>{t('Удалить пресет трека', 'Remove track preset')}</button></div>}<svg className="wa-eq-graph" width="113" height="19" viewBox="0 0 113 19" aria-label={t("Кривая эквалайзера", "Equalizer curve")}><line x1="0" y1={9 - settings.eq_preamp_db * 8 / 12} x2="113" y2={9 - settings.eq_preamp_db * 8 / 12} stroke="#ad6f58" strokeWidth="1" /><polyline points={eqPoints} fill="none" stroke="#ff9c66" strokeWidth="1" /></svg><div className="wa-eq-sliders">{[-1, ...Array.from({ length: 10 }, (_, index) => index)].map(index => <input key={index} type="range" min={-12} max={12} step={0.5} style={{ left: index < 0 ? 21 : 78 + index * 18 }} value={index < 0 ? settings.eq_preamp_db : settings.eq_gains_db[index] || 0} aria-label={index < 0 ? t('Предусиление', 'Preamp') : `${t('Полоса', 'Band')} ${index + 1}`} onChange={event => { const value = Number(event.target.value); void change(index < 0 ? 'eq_preamp_db' : 'eq_gains_db', index < 0 ? value : settings.eq_gains_db.map((gain, at) => at === index ? value : gain)) }} />)}</div></>}
      {settings.winamp_eq_shade && <div className="wa-eq-shade-controls"><input type="range" min={0} max={100} value={Math.round((player?.volume || 0) * 100)} aria-label={t("Громкость", "Volume")} onChange={event => void command('volume', Number(event.target.value) / 100)} /><input type="range" min={-1} max={1} step={0.01} value={settings.balance} aria-label={t("Баланс", "Balance")} onChange={event => void change('balance', Number(event.target.value))} /></div>}
    </section>}
    {settings.winamp_pl_window && <section className={`wa-playlist ${settings.winamp_pl_shade ? 'shade' : ''}`} style={{ top: mainHeight + eqHeight, height: listHeight }} aria-label={t("Очередь воспроизведения", "Playback queue")}><div className="wa-pl-heading"><span>PLAYLIST · {player?.queue.length || 0}</span><div><button aria-label={settings.winamp_pl_shade ? t('Развернуть очередь', 'Expand queue') : t('Свернуть очередь', 'Collapse queue')} onClick={() => void change('winamp_pl_shade', !settings.winamp_pl_shade)}>▴</button><button aria-label={t("Закрыть очередь", "Close queue")} onClick={() => void change('winamp_pl_window', false)}>×</button></div></div>{!settings.winamp_pl_shade && <><div className="wa-pl-items" style={{ height: listHeight - 58 }}>{player?.queue.map((item, index) => <button key={`${item.id}-${index}`} className={index === currentIndex ? 'active' : ''} onClick={() => void command('skip_to', undefined, index)} title={`${artist(item)} — ${item.title}`}>{index + 1}. {artist(item)} - {item.title}</button>)}</div><div className="wa-pl-footer"><button onClick={() => void command('clear_upcoming')}>{t('Очистить следующие', 'Clear upcoming')}</button><span>{clock(position)} / {clock(length)}</span><button aria-label={t("Увеличить очередь", "Increase queue height")} onClick={() => void change('winamp_pl_rows', Math.min(64, rows + 1))}>＋</button><button aria-label={t("Уменьшить очередь", "Decrease queue height")} onClick={() => void change('winamp_pl_rows', Math.max(4, rows - 1))}>－</button></div><button className="wa-pl-resize" aria-label={t('Изменить высоту очереди', 'Resize queue height')} title={t('Потяни, чтобы изменить высоту очереди', 'Drag to resize queue')} onPointerDown={event => { resizeStart.current = { y: event.clientY, rows }; event.currentTarget.setPointerCapture(event.pointerId) }} onPointerMove={event => { if (!resizeStart.current) return; setDragRows(Math.max(4, Math.min(64, resizeStart.current.rows + Math.round((event.clientY - resizeStart.current.y) / (13 * scale))))) }} onPointerUp={event => void finishResize(event.currentTarget, event.pointerId)} onPointerCancel={() => { resizeStart.current = null; setDragRows(null) }}>◢</button></>}</section>}
    {skinError && <div className="wa-error" role="alert">{t('Скин не загружен', 'Skin failed to load')}: {String(skinError)}</div>}
  </div></div>
}
