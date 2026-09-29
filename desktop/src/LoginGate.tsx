import { useState } from 'react'
import { AudioLines, ArrowRight, LoaderCircle, Music2 } from 'lucide-react'
import { useQueryClient } from '@tanstack/react-query'
import { api } from './api'
import { UpdateSettingsCard } from './Updater'
import { FASTCLOUD_SERVER_URL } from './server'
import type { Connection } from './types'

export function LoginGate({ connection, connectionError, english }: { connection?: Connection; connectionError?: string; english: boolean }) {
  const queryClient = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const pending = busy || !connection && !connectionError || !!connection && ['connecting', 'registering'].includes(connection.status)
  const action = async (run: () => Promise<void>) => {
    if (busy) return
    setBusy(true)
    setError('')
    try {
      await run()
      await queryClient.invalidateQueries({ queryKey: ['connection'] })
    } catch (cause) {
      setError(String(cause))
      await queryClient.invalidateQueries({ queryKey: ['connection'] })
    } finally {
      setBusy(false)
    }
  }

  const status = !connection && !connectionError
    ? english ? 'Checking your account…' : 'Проверяем твой аккаунт…'
    : connection?.status === 'pairing'
    ? english ? `Open the activation page and enter code ${connection.code}.` : `Открой страницу активации и введи код ${connection.code}.`
    : connection?.status === 'public'
      ? english ? 'Sign in through the Fastcloud access server with your SoundCloud account.' : 'Войди через сервер Fastcloud со своим SoundCloud-аккаунтом.'
      : pending
        ? english ? 'Complete sign-in in your browser. If access needs approval, this window will open automatically after the owner approves.' : 'Заверши вход в браузере. Если нужна заявка, приложение откроется после одобрения владельцем.'
        : english ? 'Sign in with your SoundCloud account to open Fastcloud.' : 'Войди в свой SoundCloud-аккаунт, чтобы открыть Fastcloud.'

  return <main className="login-screen"><div className="login-card">
    <div className="login-logo"><AudioLines size={24} strokeWidth={2.4} /><span>fastcloud</span></div>
    <div className="login-icon"><Music2 size={26} /></div>
    <h1>{english ? 'Your music starts here' : 'Твоя музыка начинается здесь'}</h1>
    <p className="login-description">{status}</p>
    {connection?.status === 'pairing' && <a className="login-primary" href={connection.url} target="_blank" rel="noreferrer">{english ? 'Open activation page' : 'Открыть страницу активации'} <ArrowRight size={17} /></a>}
    {connection?.status === 'public' && <button className="login-primary" disabled={pending || !FASTCLOUD_SERVER_URL} onClick={() => void action(() => api.connectServer(FASTCLOUD_SERVER_URL))}>{english ? 'Sign in with SoundCloud' : 'Войти через SoundCloud'} <ArrowRight size={17} /></button>}
    {(!connection || connection.status === 'demo' || connection.status === 'error') && <button className="login-primary" disabled={pending || !FASTCLOUD_SERVER_URL} onClick={() => void action(() => api.connectServer(FASTCLOUD_SERVER_URL))}>{pending ? <LoaderCircle className="login-spinner" size={18} /> : <ArrowRight size={17} />}{english ? 'Sign in with SoundCloud' : 'Войти через SoundCloud'}</button>}
    {pending && <div className="login-pending" role="status"><LoaderCircle className="login-spinner" size={15} />{english ? 'Waiting for connection…' : 'Ожидаем подключения…'}</div>}
    {(error || connection?.status === 'error' && connection.message) && <p className="login-error" role="alert">{error || connection?.status === 'error' && connection.message}</p>}
    {connectionError && <p className="login-error" role="alert">{connectionError}</p>}
    {!FASTCLOUD_SERVER_URL && <p className="login-error" role="alert">{english ? 'This build has no access server configured.' : 'В этой сборке не указан сервер доступа.'}</p>}
    <details className="login-advanced"><summary>{english ? 'I have my own Artist Pro app' : 'У меня есть своё приложение Artist Pro'}</summary><button disabled={pending} onClick={() => void action(connection?.status === 'public' ? api.signIn : api.connect)}>{connection?.status === 'public' ? english ? 'Sign in with my app' : 'Войти через своё приложение' : english ? 'Connect my app' : 'Подключить своё приложение'}</button></details>
    <details className="login-updates"><summary>{english ? 'App updates' : 'Обновления приложения'}</summary><UpdateSettingsCard english={english} /></details>
  </div></main>
}
