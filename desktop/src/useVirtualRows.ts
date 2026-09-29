import { useLayoutEffect, useRef, useState } from 'react'

const OVERSCAN = 8

// Keep only the rows near the viewport mounted. The list itself stays full height,
// so scroll position and indices continue to refer to the complete collection.
export function useVirtualRows(count: number, rowHeight: number, enabled: boolean, layoutKey = 0) {
  const ref = useRef<HTMLDivElement>(null)
  const [range, setRange] = useState({ start: 0, end: Math.min(count, 32) })

  useLayoutEffect(() => {
    if (!enabled) return
    const list = ref.current
    if (!list) return
    const scroller = list.closest<HTMLElement>('.scroll-area')
    let pending = 0
    const update = () => {
      pending = 0
      const listRect = list.getBoundingClientRect()
      const viewport = scroller?.getBoundingClientRect() ?? { top: 0, bottom: window.innerHeight }
      const start = Math.min(Math.max(0, count - 1), Math.max(0, Math.floor((viewport.top - listRect.top) / rowHeight) - OVERSCAN))
      const end = Math.min(count, Math.max(start + 1, Math.ceil((viewport.bottom - listRect.top) / rowHeight) + OVERSCAN))
      setRange(current => current.start === start && current.end === end ? current : { start, end })
    }
    const schedule = () => { if (!pending) pending = window.setTimeout(update, 16) }
    const observer = new ResizeObserver(schedule)
    observer.observe(scroller ?? document.documentElement)
    update()
    ;(scroller ?? window).addEventListener('scroll', schedule, { passive: true })
    window.addEventListener('resize', schedule)
    return () => {
      if (pending) clearTimeout(pending)
      observer.disconnect()
      ;(scroller ?? window).removeEventListener('scroll', schedule)
      window.removeEventListener('resize', schedule)
    }
  }, [count, rowHeight, enabled, layoutKey])

  return { ref, start: enabled ? range.start : 0, end: enabled ? range.end : count }
}
