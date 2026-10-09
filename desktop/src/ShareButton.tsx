import { useEffect } from 'react'
import { createPortal } from 'react-dom'
import { create } from 'zustand'
import { LoaderCircle, Share2, X } from 'lucide-react'
import { api } from './api'
import { shareSoundCloud, soundCloudMessagesUrl, type ShareItem, type ShareKind, type ShareOutcome } from './soundcloudShare'

type Notice = { outcome: ShareOutcome; english: boolean }
const useShare = create<{ busy: string | null; notice: Notice | null }>(() => ({ busy: null, notice: null }))

export function ShareButton({ item, kind = 'track', english = false, label = false, className = '' }: {
  item: ShareItem; kind?: ShareKind; english?: boolean; label?: boolean; className?: string
}) {
  const busy = useShare(state => state.busy)
  const key = `${kind}:${item.id}`
  const text = english ? 'Share' : 'Поделиться'
  const share = async () => {
    if (useShare.getState().busy) return
    useShare.setState({ busy: key, notice: null })
    const outcome = await shareSoundCloud(item, kind, {
      detail: (type, id) => type === 'track' ? api.track(id) : api.playlist(id),
      copy: url => navigator.clipboard.writeText(url),
      open: api.openSoundCloud,
      wait: () => new Promise(resolve => setTimeout(resolve, 300)),
    })
    useShare.setState({ busy: null, notice: { outcome, english } })
  }
  return <button type="button" className={`${label ? 'secondary-button' : 'icon-button'} share-button ${className}`} disabled={busy !== null}
    aria-label={`${text}: ${item.title}`} title={english ? 'Copy link and open SoundCloud messages' : 'Скопировать ссылку и открыть сообщения SoundCloud'}
    onClick={event => { event.stopPropagation(); void share() }}>
    {busy === key ? <LoaderCircle size={17} className="spin" /> : <Share2 size={17} />}{label && <span>{text}</span>}
  </button>
}

export function ShareNotice() {
  const notice = useShare(state => state.notice)
  useEffect(() => {
    if (notice?.outcome.status !== 'opened') return
    const timeout = setTimeout(() => useShare.setState({ notice: null }), 15_000)
    return () => clearTimeout(timeout)
  }, [notice])
  if (!notice) return null
  const { outcome, english } = notice
  const t = (ru: string, en: string) => english ? en : ru
  const message = outcome.status === 'opened' ? t('Ссылка скопирована. Выбери диалог в SoundCloud и вставь её: Ctrl+V или ⌘V.', 'Link copied. Choose a SoundCloud conversation and paste it: Ctrl+V or ⌘V.')
    : outcome.status === 'copy-failed' ? t('Не удалось скопировать ссылку. Скопируй её ниже и вставь в диалог.', 'Could not copy the link. Copy it below and paste it into a conversation.')
      : outcome.status === 'open-failed' ? t('Ссылка скопирована, но сообщения не открылись. Попробуй ещё раз.', 'Link copied, but messages did not open. Try again.')
        : t('Не удалось получить ссылку SoundCloud. Попробуй позже.', 'Could not get the SoundCloud link. Try again later.')
  return createPortal(<aside className="share-notice" aria-label={t('Поделиться музыкой', 'Share music')}>
    <p role="status">{message}</p>
    {outcome.status === 'copy-failed' && <input aria-label={t('Ссылка SoundCloud', 'SoundCloud link')} readOnly value={outcome.url} onFocus={event => event.target.select()} />}
    {(outcome.status === 'open-failed' || outcome.status === 'copy-failed') && <button className="text-button" onClick={() => void api.openSoundCloud(soundCloudMessagesUrl).catch(() => {})}>{t('Открыть сообщения SoundCloud ↗', 'Open SoundCloud messages ↗')}</button>}
    <button className="icon-button share-notice-close" aria-label={t('Закрыть', 'Close')} onClick={() => useShare.setState({ notice: null })}><X size={17} /></button>
  </aside>, document.body)
}
