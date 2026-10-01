import { useRef, useState, type PointerEvent, type ReactNode } from 'react'
import { ArrowDownUp, ChevronDown, ChevronRight, GripVertical, X } from 'lucide-react'
import { quickAccessTarget, type QuickAccessShortcut } from './types'

type Target = NonNullable<ReturnType<typeof quickAccessTarget>>
type Drag = { source: number; gap: number; x: number; y: number; pointer: number; moving: boolean }

export function QuickAccessList({ items, english, artwork, open, unpin, reorder, sort }: {
  items: QuickAccessShortcut[]
  english: boolean
  artwork: (target: Target) => ReactNode
  open: (target: Target) => void
  unpin: (item: QuickAccessShortcut) => Promise<void>
  reorder: (source: number, target: number) => Promise<void>
  sort: (items: QuickAccessShortcut[]) => Promise<void>
}) {
  const list = useRef<HTMLDivElement>(null)
  const drag = useRef<Drag | null>(null)
  const suppressClickUntil = useRef(0)
  const [position, setPosition] = useState<{ source: number; gap: number } | null>(null)
  const [saving, setSaving] = useState(false)
  const [expanded, setExpanded] = useState(() => localStorage.getItem('fastcloud:quick-access-open') !== 'false')
  const [sortOpen, setSortOpen] = useState(false)
  const rows = items.flatMap((item, index) => {
    const target = quickAccessTarget(item)
    return target ? [{ item, index, target }] : []
  })
  const saveOrder = async (source: number, target: number) => {
    if (source === target || target < 0 || target >= items.length || saving) return
    setSaving(true)
    try { await reorder(source, target) }
    finally { setSaving(false) }
  }
  const sortPins = async (mode: 'title' | 'artist' | 'kind' | 'reverse') => {
    if (saving) return
    setSortOpen(false)
    setSaving(true)
    const collator = new Intl.Collator(english ? 'en' : 'ru', { numeric: true, sensitivity: 'base' })
    const ordered = mode === 'reverse' ? [...rows].reverse() : [...rows].sort((a, b) => {
      const first = a.target[mode]
      const second = b.target[mode]
      return collator.compare(first, second) || collator.compare(a.target.title, b.target.title)
    })
    // Keep older non-displayable shortcuts in place for compatibility.
    const next = [...items]
    rows.forEach((row, index) => { next[row.index] = ordered[index].item })
    try { await sort(next) }
    finally { setSaving(false) }
  }
  const start = (event: PointerEvent<HTMLDivElement>) => {
    if (event.button !== 0 || saving || (event.target as HTMLElement).closest('[data-pin-remove]')) return
    const row = (event.target as HTMLElement).closest<HTMLElement>('[data-shortcut-index]')
    if (!row) return
    const source = Number(row.dataset.shortcutIndex)
    drag.current = { source, gap: source, x: event.clientX, y: event.clientY, pointer: event.pointerId, moving: false }
  }
  const move = (event: PointerEvent<HTMLDivElement>) => {
    const held = drag.current
    if (!held || held.pointer !== event.pointerId) return
    if (!held.moving && Math.hypot(event.clientX - held.x, event.clientY - held.y) < 6) return
    if (!held.moving) {
      held.moving = true
      event.currentTarget.setPointerCapture(event.pointerId)
    }
    event.preventDefault()
    const scroller = list.current?.closest<HTMLElement>('.sidebar-navigation-scroll')
    if (scroller) {
      const bounds = scroller.getBoundingClientRect()
      if (event.clientY < bounds.top + 28) scroller.scrollTop -= 12
      else if (event.clientY > bounds.bottom - 28) scroller.scrollTop += 12
    }
    const elements = list.current?.querySelectorAll<HTMLElement>('[data-shortcut-index]')
    let gap = rows.at(-1)!.index + 1
    for (const row of elements || []) {
      const bounds = row.getBoundingClientRect()
      if (event.clientY < bounds.top + bounds.height / 2) { gap = Number(row.dataset.shortcutIndex); break }
    }
    held.gap = gap
    setPosition({ source: held.source, gap })
  }
  const finish = (event: PointerEvent<HTMLDivElement>, cancelled = false) => {
    const held = drag.current
    if (!held || held.pointer !== event.pointerId) return
    drag.current = null
    setPosition(null)
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
    if (!held.moving) return
    suppressClickUntil.current = performance.now() + 350
    if (!cancelled) void saveOrder(held.source, held.gap > held.source ? held.gap - 1 : held.gap)
  }
  if (!rows.length) return null
  return <div ref={list} className={`quick-access ${position ? 'is-reordering' : ''}`} aria-busy={saving}
    onPointerDown={start} onPointerMove={move} onPointerUp={event => finish(event)} onPointerCancel={event => finish(event, true)}
    onDragStart={event => event.preventDefault()} onClickCapture={event => {
      if (performance.now() < suppressClickUntil.current) { event.preventDefault(); event.stopPropagation() }
    }}>
    <div className="quick-access-heading"><button className="quick-access-header" aria-expanded={expanded} onClick={() => setExpanded(value => {
      localStorage.setItem('fastcloud:quick-access-open', String(!value))
      return !value
    })}>{expanded ? <ChevronDown size={15} /> : <ChevronRight size={15} />}<span>{english ? 'Quick access' : 'Быстрый доступ'}</span><small>{rows.length}</small></button><button className="quick-access-sort" disabled={saving} aria-label={english ? 'Sort pins' : 'Сортировка закреплений'} aria-expanded={sortOpen} title={english ? 'Sort pins' : 'Сортировка закреплений'} onClick={() => setSortOpen(value => !value)}><ArrowDownUp size={15} /></button>{sortOpen && <div className="quick-access-sort-menu"><button onClick={() => void sortPins('title')}>{english ? 'By title' : 'По названию'}</button><button onClick={() => void sortPins('artist')}>{english ? 'By artist' : 'По исполнителю'}</button><button onClick={() => void sortPins('kind')}>{english ? 'By type' : 'По типу'}</button><button onClick={() => void sortPins('reverse')}>{english ? 'Reverse order' : 'Обратный порядок'}</button><button onClick={() => setSortOpen(false)}>{english ? 'Manual order · drag' : 'Вручную · перетаскивай'}</button></div>}</div>
    {expanded && rows.map(({ item, index, target }, slot) => <div key={`${target.kind}-${target.id}`} data-shortcut-index={index}
      className={`quick-access-row ${position?.source === index ? 'is-dragging' : ''} ${position?.gap === index ? 'drop-before' : ''} ${position && slot === rows.length - 1 && position.gap === index + 1 ? 'drop-after' : ''}`}>
      <button className="quick-access-grip" disabled={saving} aria-label={`${english ? 'Reorder' : 'Переместить'} ${target.title}`}
        title={english ? 'Drag, or use ↑ / ↓ to reorder' : 'Перетащи мышью или используй ↑ / ↓'}
        onKeyDown={event => {
          if (event.key !== 'ArrowUp' && event.key !== 'ArrowDown') return
          event.preventDefault()
          const destination = rows[slot + (event.key === 'ArrowUp' ? -1 : 1)]
          if (destination) void saveOrder(index, destination.index)
        }}><GripVertical size={14} /></button>
      <button className="quick-access-open" title={target.title} onClick={() => open(target)}>{artwork(target)}<span><strong>{target.title}</strong><small>{target.artist}</small></span></button>
      <button className="quick-access-remove" data-pin-remove disabled={saving} title={english ? 'Unpin' : 'Открепить'} aria-label={`${english ? 'Unpin' : 'Открепить'} ${target.title}`} onClick={() => void unpin(item)}><X size={14} /></button>
    </div>)}
  </div>
}
