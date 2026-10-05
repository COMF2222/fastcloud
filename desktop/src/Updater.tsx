import { createContext, useContext, useEffect, useRef, useState, type ReactNode } from 'react'
import { getVersion } from '@tauri-apps/api/app'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'
import { useQuery } from '@tanstack/react-query'
import { api } from './api'
import { displayReleaseVersion, releaseIsNewer, startUpdatePolling } from './updatePolling'
import { FASTCLOUD_SERVER_URL } from './server'

type UpdateState = {
  version: string
  available: Update | null
  checking: boolean
  installing: boolean
  progress: number | null
  error: string
  checked: boolean
  dismissedVersion: string
  checkNow: () => Promise<void>
  install: () => Promise<void>
  dismiss: () => void
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
  const [dismissedVersion, setDismissedVersion] = useState('')
  const busy = useRef(false)
  const availableRef = useRef<Update | null>(null)
  const mounted = useRef(false)
  const pendingCheck = useRef(false)
  const installedVersion = useRef('')
  const announcedVersion = useRef('')
  const retryAttempts = useRef(0)
  const retryTimer = useRef<ReturnType<typeof setTimeout> | null>(null)

  const checkNow = async () => {
    if (api.preview) return
    if (busy.current) { pendingCheck.current = true; return }
    if (retryTimer.current) clearTimeout(retryTimer.current)
    retryTimer.current = null
    busy.current = true
    setChecking(true)
    setError('')
    try {
      installedVersion.current = await getVersion()
      setVersion(installedVersion.current)
      const next = await check({ timeout: 15_000, headers: { 'Cache-Control': 'no-cache' } })
      if (!mounted.current) {
        if (next) void next.close().catch(() => {})
        return
      }
      const previous = availableRef.current
      availableRef.current = next
      setAvailable(next)
      if (previous) void previous.close().catch(() => {})
      setChecked(true)
    } catch (cause) {
      setError(String(cause))
    } finally {
      setChecking(false)
      busy.current = false
      if (mounted.current && pendingCheck.current) {
        pendingCheck.current = false
        void checkNow()
      } else if (mounted.current && retryAttempts.current > 0
        && releaseIsNewer(announcedVersion.current, installedVersion.current)
        && releaseIsNewer(announcedVersion.current, availableRef.current?.version || installedVersion.current)) {
        // Release assets can take a moment to reach GitHub's download endpoint.
        retryAttempts.current--
        retryTimer.current = setTimeout(() => void checkNow(), 10_000)
      }
    }
  }

  const install = async () => {
    const update = availableRef.current
    if (!update || busy.current) return
    busy.current = true
    setInstalling(true)
    setError('')
    let downloaded = 0
    let total = 0
    try {
      // Preserve bundled files before NSIS replaces the installation. A failed
      // optional model setup must not prevent updating a broken client.
      try { await invoke('preserve_components') } catch { /* The next client can retry setup. */ }
      await update.downloadAndInstall(event => {
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
    mounted.current = true
    const polling = startUpdatePolling(() => void checkNow())
    let disposed = false
    let unlisten: (() => void) | undefined
    let unlistenRelease: (() => void) | undefined
    if (FASTCLOUD_SERVER_URL) {
      void listen<string>('release-published', ({ payload }) => {
        if (disposed || (installedVersion.current && !releaseIsNewer(payload, installedVersion.current))) return
        if (announcedVersion.current && !releaseIsNewer(payload, announcedVersion.current)) return
        announcedVersion.current = payload
        retryAttempts.current = 6
        polling.published()
      }).then(stop => {
        if (disposed) { stop(); return }
        unlistenRelease = stop
        return invoke<void>('subscribe_updates', { serverUrl: FASTCLOUD_SERVER_URL })
      }).catch(() => {})
    }
    void getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (!disposed && focused) polling.resume()
    }).then(stop => {
      if (disposed) stop()
      else unlisten = stop
    }).catch(() => {})
    return () => {
      mounted.current = false
      disposed = true
      polling.stop()
      unlisten?.()
      unlistenRelease?.()
      if (retryTimer.current) clearTimeout(retryTimer.current)
      pendingCheck.current = false
      const previous = availableRef.current
      availableRef.current = null
      if (previous) void previous.close().catch(() => {})
    }
  }, [])

  return <Context.Provider value={{ version, available, checking, installing, progress, error, checked, dismissedVersion, checkNow, install, dismiss: () => setDismissedVersion(available?.version || '') }}>{children}</Context.Provider>
}

function useUpdater() {
  const state = useContext(Context)
  if (!state) throw new Error('UpdateProvider is missing')
  return state
}

export function UpdateSidebarButton({ english, openSettings }: { english: boolean; openSettings: () => void }) {
  const { available } = useUpdater()
  if (!available) return null
  const label = displayReleaseVersion(available.version)
  return <button className="nav-item update-sidebar-button" onClick={openSettings} title={english ? `Update to ${label}` : `Обновить до ${label}`}>
    <span aria-hidden="true">↑</span><span>{english ? `Update available: ${label}` : `Доступно обновление ${label}`}</span>
  </button>
}

export function UpdateNotice({ english }: { english: boolean }) {
  const { available, dismissedVersion, installing, progress, error, install, dismiss } = useUpdater()
  if (!available || dismissedVersion === available.version) return null
  const label = displayReleaseVersion(available.version)
  return <div className="update-notice" role="status">
    <div><strong>{english ? `Fastcloud ${label} is available` : `Доступна новая версия Fastcloud ${label}`}</strong><span>{english ? 'You can install it now.' : 'Её можно установить прямо сейчас.'}</span>{error && <small role="alert">{error}</small>}</div>
    <div className="update-actions">
      <button className="primary-button" disabled={installing} onClick={() => void install()}>{installing ? english ? `Installing… ${progress == null ? '' : `${progress}%`}` : `Устанавливаем… ${progress == null ? '' : `${progress}%`}` : english ? 'Update now' : 'Обновить'}</button>
      <ReleaseNotesButton version={available.version} english={english} />
    </div>
    <button className="icon-button" disabled={installing} aria-label={english ? 'Remind me next launch' : 'Напомнить при следующем запуске'} title={english ? 'Remind me next launch' : 'Напомнить при следующем запуске'} onClick={dismiss}>×</button>
  </div>
}

function ReleaseNotesButton({ version, english }: { version: string; english: boolean }) {
  const [opening, setOpening] = useState(false)
  const [failed, setFailed] = useState(false)
  const open = async () => {
    setOpening(true)
    setFailed(false)
    try { await api.openReleaseNotes(version, english) }
    catch { setFailed(true) }
    finally { setOpening(false) }
  }
  return <div className="release-notes-action">
    <button className="secondary-button" disabled={opening} onClick={() => void open()} title={english ? 'Open release notes in your browser' : 'Открыть описание обновления в браузере'}>
      {english ? 'Read changes' : 'Прочитать изменения'}
    </button>
    {failed && <small className="error-text" role="alert">{english ? 'Could not open the browser. Try again.' : 'Не удалось открыть браузер. Попробуй ещё раз.'}</small>}
  </div>
}

export function UpdateSettingsCard({ english }: { english: boolean }) {
  const { version, available, checking, installing, progress, error, checked, checkNow, install } = useUpdater()
  return <div className="settings-card update-settings-card">
    <h3>{english ? 'App updates' : 'Обновления приложения'}</h3>
    <p>{english ? 'Installed version' : 'Установленная версия'}: {displayReleaseVersion(version) || '…'}</p>
    {available ? <>
      <p role="status">{english ? `Version ${displayReleaseVersion(available.version)} is available.` : `Доступна версия ${displayReleaseVersion(available.version)}.`}</p>
      <div className="update-actions">
        <button className="secondary-button" disabled={installing} onClick={() => void install()}>
          {installing ? english ? `Installing… ${progress == null ? '' : `${progress}%`}` : `Устанавливаем… ${progress == null ? '' : `${progress}%`}` : english ? 'Update now' : 'Обновить сейчас'}
        </button>
        <ReleaseNotesButton version={available.version} english={english} />
      </div>
    </> : <>
      {checked && <p role="status">{english ? 'You have the latest version.' : 'У вас последняя версия.'}</p>}
      <button className="secondary-button" disabled={checking || installing} onClick={() => void checkNow()}>
        {checking ? english ? 'Checking…' : 'Проверяем…' : english ? 'Check for updates' : 'Проверить обновления'}
      </button>
    </>}
    {error && <p className="error-text" role="alert">{english ? 'Update failed' : 'Не удалось обновить'}: {error}</p>}
    <ComponentStatusCard english={english} />
  </div>
}

type ComponentStatus = { state: 'unavailable' | 'checking' | 'downloading' | 'ready' | 'error';
  completedFiles: number; totalFiles: number; downloadedBytes: number; error: string | null }

function ComponentStatusCard({ english }: { english: boolean }) {
  const { data, refetch } = useQuery({ queryKey: ['component-status'],
    queryFn: () => invoke<ComponentStatus>('component_status'), enabled: !api.preview,
    refetchInterval: query => ['checking', 'downloading'].includes(query.state.data?.state || '') ? 1000 : 10_000,
    retry: false })
  const [retrying, setRetrying] = useState(false)
  if (!data || data.state === 'unavailable' || data.state === 'ready') return null
  const preparing = data.state === 'checking' || data.state === 'downloading'
  const retry = async () => {
    setRetrying(true)
    try { await invoke('prepare_components') } catch { /* The status response contains the error. */ }
    finally { setRetrying(false); void refetch() }
  }
  return <div className="component-status" role="status">
    <p>{preparing
        ? `${english ? 'Preparing recommendation model' : 'Подготовка модели рекомендаций'}: ${data.completedFiles}/${data.totalFiles}`
        : english ? 'Could not prepare the recommendation model. Music playback remains available.' : 'Не удалось подготовить модель рекомендаций. Музыку по-прежнему можно слушать.'}</p>
    {data.state === 'error' && <button className="secondary-button" disabled={retrying} onClick={() => void retry()}>
      {english ? 'Retry' : 'Повторить'}</button>}
  </div>
}
