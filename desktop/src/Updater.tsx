import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { api } from './api'

type UpdateState = {
  version: string
  available: Update | null
  checking: boolean
  installing: boolean
  progress: number | null
  error: string
  checked: boolean
  checkNow: () => Promise<void>
  install: () => Promise<void>
}

const Context = createContext<UpdateState | null>(null)

export function UpdateProvider({ children }: { children: ReactNode }) {
  const [version, setVersion] = useState('')
  const [available, setAvailable] = useState<Update | null>(null)
  const [checking, setChecking] = useState(false)
  const [installing, setInstalling] = useState(false)
  const [progress, setProgress] = useState<number | null>(null)
  const [error, setError] = useState('')
  const [checked, setChecked] = useState(false)
  const busy = useRef(false)

  const checkNow = async () => {
    if (api.preview || busy.current) return
    busy.current = true
    setChecking(true)
    setError('')
    try {
      setVersion(await getVersion())
      setAvailable(await check())
      setChecked(true)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setChecking(false)
      busy.current = false
    }
  }

  const install = async () => {
    if (!available || busy.current) return
    busy.current = true
    setInstalling(true)
    setError('')
    let downloaded = 0
    let total = 0
    try {
      await available.downloadAndInstall(event => {
        if (event.event === 'Started') {
          total = event.data.contentLength ?? 0
          setProgress(0)
        } else if (event.event === 'Progress') {
          downloaded += event.data.chunkLength
          if (total) setProgress(Math.min(100, Math.round(downloaded / total * 100)))
        } else if (event.event === 'Finished') {
          setProgress(100)
        }
      })
      await relaunch()
    } catch (cause) {
      setError(String(cause))
      setInstalling(false)
      busy.current = false
    }
  }

  useEffect(() => {
    if (api.preview) return
    void checkNow()
    const timer = window.setInterval(() => void checkNow(), 6 * 60 * 60 * 1000)
    return () => window.clearInterval(timer)
  }, [])

  return <Context.Provider value={{ version, available, checking, installing, progress, error, checked, checkNow, install }}>{children}</Context.Provider>
}

function useUpdater() {
  const state = useContext(Context)
  if (!state) throw new Error('UpdateProvider is missing')
  return state
}

export function UpdateSidebarButton({ english, openSettings }: { english: boolean; openSettings: () => void }) {
  const { available } = useUpdater()
  if (!available) return null
  return <button className="nav-item update-sidebar-button" onClick={openSettings} title={english ? `Update to ${available.version}` : `Обновить до ${available.version}`}>
    <span aria-hidden="true">↑</span><span>{english ? `Update available: ${available.version}` : `Доступно обновление ${available.version}`}</span>
  </button>
}

export function UpdateSettingsCard({ english }: { english: boolean }) {
  const { version, available, checking, installing, progress, error, checked, checkNow, install } = useUpdater()
  return <div className="settings-card update-settings-card">
    <h3>{english ? 'App updates' : 'Обновления приложения'}</h3>
    <p>{english ? 'Installed version' : 'Установленная версия'}: {version || '…'}</p>
    {available ? <>
      <p role="status">{english ? `Version ${available.version} is available.` : `Доступна версия ${available.version}.`}</p>
      <button className="secondary-button" disabled={installing} onClick={() => void install()}>
        {installing ? english ? `Installing… ${progress == null ? '' : `${progress}%`}` : `Устанавливаем… ${progress == null ? '' : `${progress}%`}` : english ? 'Update now' : 'Обновить сейчас'}
      </button>
    </> : <>
      {checked && <p role="status">{english ? 'You have the latest version.' : 'У вас последняя версия.'}</p>}
      <button className="secondary-button" disabled={checking || installing} onClick={() => void checkNow()}>
        {checking ? english ? 'Checking…' : 'Проверяем…' : english ? 'Check for updates' : 'Проверить обновления'}
      </button>
    </>}
    {error && <p className="error-text" role="alert">{english ? 'Update failed' : 'Не удалось обновить'}: {error}</p>}
  </div>
}
