import { useEffect, useRef, useState } from 'react'
import type { Settings } from './types'
import { applyThemeCustomization, colorHex, colorRgb, headingSurfaceOpacity, interfaceLimits, themeDefaults } from './theme'
import { ColorPicker } from './ColorPicker'

type Update = (key: string, value: unknown) => Promise<void>
type ColorKey = 'panel_rgb' | 'text_rgb' | 'muted_text_rgb'
type RangeKey = 'panel_opacity' | 'panel_blur' | 'heading_opacity' | 'interface_text_scale' | 'interface_scale'

function ColorSetting({ label, field, fallback, settings, update }: { label: string; field: ColorKey; fallback: number[]; settings: Settings; update: Update }) {
  const saved = settings[field] ? colorHex(settings[field]) : colorHex(fallback)
  const [draft, setDraft] = useState(saved)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const pending = useRef<string | null>(null)
  const latestUpdate = useRef(update)
  latestUpdate.current = update
  const commit = () => {
    if (timer.current) clearTimeout(timer.current)
    if (pending.current === null) return
    const value = pending.current
    pending.current = null
    void latestUpdate.current(field, colorRgb(value))
  }
  useEffect(() => { if (pending.current === null) setDraft(saved) }, [saved])
  useEffect(() => () => commit(), [])
  const preview = (value: string) => {
    setDraft(value)
    if (!/^#[0-9a-f]{6}$/i.test(value)) return
    pending.current = value
    applyThemeCustomization({ ...settings, [field]: colorRgb(value) })
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(commit, 200)
  }
  return <div className="theme-color-setting"><div className="field-label"><label htmlFor={`theme-${field}`}>{label}</label><div className="theme-color-row"><div className="theme-color-input"><ColorPicker id={`theme-${field}`} value={draft} label={label} english={settings.language === 'English'} onChange={preview} onCommit={() => commit()} />{!settings[field] && <span>{settings.language === 'English' ? 'From theme' : 'Как в теме'}</span>}</div><button className="text-button" disabled={!settings[field]} onClick={() => { if (timer.current) clearTimeout(timer.current); pending.current = null; void update(field, null) }}>{settings.language === 'English' ? 'Auto' : 'Авто'}</button></div></div></div>
}

function RangeSetting({ field, label, min, max, step, settings, update, suffix = '%' }: { field: RangeKey; label: string; min: number; max: number; step: number; settings: Settings; update: Update; suffix?: string }) {
  const changesLayout = field === 'interface_text_scale' || field === 'interface_scale'
  const fallback = field === 'heading_opacity' ? headingSurfaceOpacity(settings) : themeDefaults[field]
  const saved = Math.max(min, Math.min(max, settings[field] ?? fallback))
  const [draft, setDraft] = useState(saved)
  const pending = useRef<number | null>(null)
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null)
  const latestUpdate = useRef(update)
  latestUpdate.current = update
  const commit = () => {
    if (timer.current) clearTimeout(timer.current)
    if (pending.current === null) return
    const value = pending.current
    pending.current = null
    void latestUpdate.current(field, value)
  }
  useEffect(() => { if (pending.current === null) setDraft(saved) }, [saved])
  useEffect(() => () => commit(), [])
  return <div className="theme-range-control"><label className="field-label theme-range"><span>{label}<output>{Math.round(suffix === '%' ? draft * 100 : draft)}{suffix}</output></span><input aria-label={label} type="range" min={min} max={max} step={step} value={draft} onPointerDown={event => event.currentTarget.setPointerCapture(event.pointerId)} onChange={event => {
    const value = Number(event.target.value)
    setDraft(value); pending.current = value
    // Resizing the page during a drag moves the control beneath the pointer and
    // rerasterizes every label. Keep the geometry stable until the gesture ends.
    if (changesLayout) return
    applyThemeCustomization({ ...settings, [field]: value })
    if (timer.current) clearTimeout(timer.current)
    timer.current = setTimeout(commit, 200)
  }} onPointerUp={commit} onPointerCancel={commit} onKeyUp={commit} onBlur={commit} /></label>{changesLayout && <div className="theme-size-preview" aria-label={settings.language === 'English' ? 'Size preview' : 'Пример размера'}>
    {field === 'interface_text_scale' ? <span style={{ fontSize: `${14 * draft}px` }}>{settings.language === 'English' ? 'Your music, your style' : 'Твоя музыка, твой стиль'}</span> : <span className="theme-element-preview" style={{ padding: `${6 * draft}px ${12 * draft}px`, gap: `${8 * draft}px` }}><span className="theme-element-preview-art" style={{ width: `${20 * draft}px`, height: `${20 * draft}px` }}>♪</span>{settings.language === 'English' ? 'Track' : 'Трек'}</span>}
  </div>}</div>
}

export function ThemeSettings({ settings, update }: { settings: Settings; update: Update }) {
  const [resetting, setResetting] = useState(false)
  const t = (ru: string, en: string) => settings.language === 'English' ? en : ru
  const light = document.documentElement.dataset.theme === 'light'
  const reset = async () => {
    setResetting(true)
    try { for (const [key, value] of Object.entries(themeDefaults)) await update(key, value) }
    finally { setResetting(false) }
  }
  return <>
    <div className="settings-card"><h3>{t('Текст и размеры интерфейса', 'Interface text and size')}</h3><p>{t('Размер текста меняется во всех разделах, меню и плеере. Размер элементов увеличивает кнопки, отступы, обложки и строки. Текст песен настраивается отдельно ниже.', 'Text size applies to pages, menus and the player. Element size changes buttons, spacing, artwork and rows. Lyrics size has its own setting below.')}</p>
      <p className="muted">{t('Во время перетаскивания размер виден на примере. Интерфейс изменится, когда отпустишь ползунок.', 'Preview the size while dragging. The interface updates when you release the slider.')}</p>
      <div className="form-grid theme-size-controls"><RangeSetting field="interface_text_scale" label={t('Размер текста интерфейса', 'Interface text size')} min={interfaceLimits.interface_text_scale[0]} max={interfaceLimits.interface_text_scale[1]} step={.01} settings={settings} update={update} /><RangeSetting field="interface_scale" label={t('Размер элементов', 'Element size')} min={interfaceLimits.interface_scale[0]} max={interfaceLimits.interface_scale[1]} step={.01} settings={settings} update={update} /></div>
      <div className="form-grid theme-text-colors">
        <ColorSetting field="text_rgb" label={t('Основной текст', 'Primary text')} fallback={light ? [32, 36, 49] : [241, 241, 244]} settings={settings} update={update} /><ColorSetting field="muted_text_rgb" label={t('Вторичный текст', 'Secondary text')} fallback={light ? [85, 93, 110] : [189, 193, 205]} settings={settings} update={update} /></div>
    </div>
    <div className="settings-card"><h3>{t('Подложки', 'Surfaces')}</h3><p>{t('Сайдбар, навигация настроек, панели, списки, меню и плеер используют общий цвет. Непрозрачность подложек не меняет затемнение обоев.', 'The sidebar, settings navigation, panels, lists, menus and player share this colour. Surface opacity is independent of wallpaper dimming.')}</p>
      <label className="setting-row"><span><strong>{t('Свои подложки', 'Custom surfaces')}</strong><small>{t('Выключи, чтобы вернуть подложки выбранной темы.', 'Turn off to use the selected theme’s surfaces.')}</small></span><input type="checkbox" checked={!!settings.panel_rgb} onChange={event => void update('panel_rgb', event.target.checked ? light ? [249, 250, 245] : [18, 20, 29] : null)} /></label>
      {settings.panel_rgb && <><div className="form-grid"><ColorSetting field="panel_rgb" label={t('Цвет подложек', 'Surface colour')} fallback={[18, 20, 29]} settings={settings} update={update} /></div><div className="form-grid"><RangeSetting field="panel_opacity" label={t('Непрозрачность подложек', 'Surface opacity')} min={0} max={1} step={.05} settings={settings} update={update} /><RangeSetting field="panel_blur" label={t('Размытие под подложками', 'Surface blur')} min={0} max={40} step={1} suffix=" px" settings={settings} update={update} /></div></>}
      <div className="theme-heading-opacity"><RangeSetting field="heading_opacity" label={t('Подложки заголовков', 'Heading surface opacity')} min={0} max={1} step={.05} settings={settings} update={update} /><button className="text-button" disabled={settings.heading_opacity == null} onClick={() => void update('heading_opacity', null)}>{t('Авто', 'Auto')}</button></div>
      <p className="muted">{t('Отдельная прозрачность блоков вроде «Настройки». На 0% заливка и размытие исчезают, текст остаётся видимым. «Авто» возвращает подложки темы.', 'Separate opacity for page headings such as Settings. At 0%, the fill and blur disappear while text stays visible. Auto restores the theme’s surfaces.')}</p>
      <p className="muted">{t('В экономном профиле размытие отключено для снижения нагрузки.', 'The Eco profile disables blur to reduce resource use.')}</p>
      <div className="theme-preview" aria-label={t('Пример оформления', 'Theme preview')}><strong>{t('Так выглядит ваш текст', 'Your text looks like this')}</strong><span>{t('Исполнитель · Альбом · Дополнительная информация', 'Artist · Album · Additional information')}</span><div><button className="secondary-button" type="button">{t('Пример кнопки', 'Example button')}</button><span className="theme-preview-accent">{t('Акцент', 'Accent')}</span></div></div>
      <button className="secondary-button" disabled={resetting} onClick={() => void reset()}>{t('Сбросить подложки, цвета текста и размеры', 'Reset surfaces, text colours and sizes')}</button>
    </div>
  </>
}
