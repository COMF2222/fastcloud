import { useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { ChevronDown } from 'lucide-react'
import { useApp } from './store'
import { artist, creditedArtists, sameArtistName, type Track } from './types'

export function ArtistCredits({ track, className, title, onNavigate, english = false }: { track: Track; className?: string; title?: string; onNavigate?: () => void; english?: boolean }) {
  const names = creditedArtists(track)
  const credit = artist(track)
  const [open, setOpen] = useState(false)
  const [position, setPosition] = useState({ top: 0, left: 0 })
  const trigger = useRef<HTMLButtonElement>(null)
  const menu = useRef<HTMLDivElement>(null)
  useEffect(() => {
    if (!open) return
    const dismiss = (event: PointerEvent) => {
      if (!trigger.current?.contains(event.target as Node) && !menu.current?.contains(event.target as Node)) setOpen(false)
    }
    const close = () => setOpen(false)
    document.addEventListener('pointerdown', dismiss)
    window.addEventListener('scroll', close, true)
    window.addEventListener('resize', close)
    return () => { document.removeEventListener('pointerdown', dismiss); window.removeEventListener('scroll', close, true); window.removeEventListener('resize', close) }
  }, [open])
  const navigate = (name: string) => {
    setOpen(false)
    onNavigate?.()
    if (track.user?.id && sameArtistName(name, track.user.username)) useApp.getState().openArtist(track.user.id, track.user.username)
    else useApp.getState().openArtistLookup(name)
  }
  const toggle = () => {
    if (names.length < 2) { navigate(names[0] || credit); return }
    if (open) { setOpen(false); return }
    const rect = trigger.current!.getBoundingClientRect()
    const height = Math.min(360, 56 + (names.length + 1) * 43)
    setPosition({ top: rect.bottom + height + 8 <= window.innerHeight ? rect.bottom + 6 : Math.max(8, rect.top - height - 6), left: Math.max(8, Math.min(rect.left, window.innerWidth - 288)) })
    setOpen(true)
  }
  return <span className="artist-credit-control">
    <button ref={trigger} className={className} title={title || credit} aria-haspopup={names.length > 1 ? 'menu' : undefined} aria-expanded={names.length > 1 ? open : undefined} aria-label={names.length > 1 ? `${english ? 'Choose artist' : 'Выбрать исполнителя'}: ${credit}` : undefined} onClick={toggle}>{credit}{names.length > 1 && <ChevronDown className="artist-credit-chevron" size={13} aria-hidden="true" />}</button>
    {open && createPortal(<div ref={menu} className="artist-credit-menu" role="menu" aria-label={english ? 'Artists on this track' : 'Исполнители трека'} style={{ top: position.top, left: position.left }} onKeyDown={event => { if (event.key === 'Escape') { setOpen(false); trigger.current?.focus() } }}><span className="artist-credit-menu-label">{english ? 'Artists on this track' : 'Исполнители трека'}</span>{names.map(name => <button key={name} role="menuitem" onClick={() => navigate(name)}>{name}</button>)}{track.user?.id && !names.some(name => sameArtistName(name, track.user!.username)) && <button className="artist-credit-uploader" role="menuitem" onClick={() => navigate(track.user!.username)}>{english ? 'Uploader' : 'Загрузил'}: {track.user.username}</button>}</div>, document.body)}
  </span>
}
