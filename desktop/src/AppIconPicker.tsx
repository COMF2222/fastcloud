import { useEffect, useState, useSyncExternalStore } from 'react'
import { iconRequest, supportsSystemIcon } from './iconBridge'
import './appIcons.css'

type State = { selected: number; supported: boolean; note?: string | null }
const labels = [["Течение", "Flow"], ["Вектор", "Vector"], ["Импульс", "Pulse"], ["Баланс", "Balance"], ["Веер", "Fan"], ["Ритм", "Rhythm"], ["Искра", "Spark"], ["Орбита", "Orbit"], ["Луна", "Moon"], ["Контур", "Contour"], ["Лента", "Ribbon"], ["Поток", "Stream"]]
let state: State = { selected: 9, supported: supportsSystemIcon }
let loaded = false
const listeners = new Set<() => void>()
function publish(value: State) {
  if (!Number.isInteger(value.selected) || value.selected < 1 || value.selected > 12) throw new Error('Invalid app icon')
  state = value
  for (const listener of listeners) listener()
}
const subscribe = (listener: () => void) => { listeners.add(listener); return () => { listeners.delete(listener) } }
export function useAppIcon() {
  const view = useSyncExternalStore(subscribe, () => state)
  useEffect(() => {
    if (loaded) return
    loaded = true
    void iconRequest().then(publish).catch(() => { loaded = false })
  }, [])
  return `/app-icons/${String(view.selected).padStart(2, '0')}.png`
}
export function AppIconPicker({ english = false }: { english?: boolean }) {
  useAppIcon()
  const view = useSyncExternalStore(subscribe, () => state)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const t = (ru: string, en: string) => english ? en : ru
  async function choose(id: number) {
    if (busy || id === state.selected) return
    setBusy(true); setError('')
    try { publish(await iconRequest(id)) }
    catch { setError(t('Не удалось изменить значок. Попробуй ещё раз.', 'Could not change the icon. Try again.')) }
    finally { setBusy(false) }
  }
  return <section className="settings-card app-icon-settings" aria-busy={busy}>
    <h2>{t('Значок приложения', 'App icon')}</h2>
    <p className="app-icon-caption">{view.supported ? t('Выбери значок для этого устройства.', 'Choose an icon for this device.') : t('Предпросмотр. Системный значок меняется в приложении.', 'Preview. System icons can be changed in the app.')}</p>
    <div className="app-icon-grid" role="group" aria-label={t('Значок приложения', 'App icon')}>
      {labels.map((label, index) => <button type="button" key={index} className="app-icon-choice" disabled={busy} aria-pressed={view.selected === index + 1} onClick={() => void choose(index + 1)}>
        <img src={`/app-icons/${String(index + 1).padStart(2, '0')}.png`} alt="" width="64" height="64" loading="lazy" />
        <span>{index + 1}. {label[english ? 1 : 0]}</span>
      </button>)}
    </div>
    <p className="app-icon-feedback" role="status">{error || (busy ? t('Меняем значок…', 'Changing icon…') : view.note === 'finder-readonly' ? t('Значок в Dock изменён. Значок в Finder недоступен для этого расположения приложения.', 'Dock icon changed. Finder icon cannot be updated at this application location.') : view.note === 'window-tray-only' ? t('Изменены значки окна и трея.', 'Window and tray icons changed.') : view.note || '')}</p>
  </section>
}
