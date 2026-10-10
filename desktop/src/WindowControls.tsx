import { useEffect, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { Copy, Minus, Square, X } from 'lucide-react'
import { api } from './api'
import { windowAction } from './windowActions'
import './windowControls.css'

export function WindowControls() {
  const { data: settings } = useQuery({ queryKey: ['settings'], queryFn: api.settings })
  const mini = !!settings?.winamp_window
  const english = settings?.language === 'English'
  const [maximized, setMaximized] = useState(false)
  const [error, setError] = useState(false)
  const t = (ru: string, en: string) => english ? en : ru
  const action = async (kind: 'minimize' | 'maximize' | 'close') => {
    if (api.preview) return
    setError(false)
    try {
      const window = getCurrentWindow()
      await windowAction(window, kind, mini)
      if (kind === 'maximize') setMaximized(await window.isMaximized() || await window.isFullscreen())
    } catch { setError(true) }
  }
  useEffect(() => {
    document.documentElement.dataset.customWindow = 'true'
    if (api.preview) return
    const window = getCurrentWindow()
    let disposed = false
    let unlisten: (() => void) | undefined
    const refresh = () => void Promise.all([window.isMaximized(), window.isFullscreen()]).then(([maximized, fullscreen]) => { if (!disposed) setMaximized(maximized || fullscreen) }).catch(() => {})
    refresh()
    void window.onResized(refresh).then(stop => { if (disposed) stop(); else unlisten = stop }).catch(() => {})
    return () => { disposed = true; unlisten?.() }
  }, [])
  useEffect(() => {
    if (api.preview) return
    const drag = (event: MouseEvent) => {
      const target = event.target instanceof Element ? event.target : null
      if (event.button !== 0 || !target?.closest('.topbar, .sidebar-toolbar, .lyrics-panel-header, .now-playing-header, .login-screen, .mini-player .player-bar, [data-window-drag]')) return
      if (target.closest('button, a, input, textarea, select, label, [role="separator"], .login-card')) return
      if (event.detail === 2 && !mini) { void action('maximize'); return }
      void getCurrentWindow().startDragging().catch(() => setError(true))
    }
    document.addEventListener('mousedown', drag)
    return () => document.removeEventListener('mousedown', drag)
  }, [mini])
  return <div className={`window-controls${mini ? ' compact' : ''}`} role="group" aria-label={t('Управление окном', 'Window controls')}>
    <button type="button" aria-label={t('Свернуть окно', 'Minimize window')} title={t('Свернуть', 'Minimize')} onClick={() => void action('minimize')}><Minus size={16} /></button>
    {!mini && <button type="button" aria-label={maximized ? t('Восстановить размер окна', 'Restore window') : t('Развернуть окно', 'Maximize window')} title={maximized ? t('Восстановить', 'Restore') : t('Развернуть', 'Maximize')} onClick={() => void action('maximize')}>{maximized ? <Copy size={14} /> : <Square size={14} />}</button>}
    <button type="button" className="window-close" aria-label={t('Закрыть окно приложения', 'Close application window')} title={t('Закрыть', 'Close')} onClick={() => void action('close')}><X size={17} /></button>
    {error && <span className="window-action-error" role="alert">{t('Не удалось изменить окно', 'Could not change the window')}</span>}
  </div>
}
