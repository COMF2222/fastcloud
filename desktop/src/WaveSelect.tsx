import { useEffect, useId, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { Check, ChevronDown } from 'lucide-react'

type Choice = readonly [value: string, ru: string, en: string]

export function WaveSelect({ label, value, choices, english, disabled, onChange }: { label: string; value: string; choices: readonly Choice[]; english: boolean; disabled: boolean; onChange: (value: string) => void }) {
  const id = useId()
  const trigger = useRef<HTMLButtonElement>(null)
  const menu = useRef<HTMLDivElement>(null)
  const [open, setOpen] = useState(false)
  const [position, setPosition] = useState({ left: 0, top: 0, width: 260 })
  const selected = Math.max(0, choices.findIndex(choice => choice[0] === value))
  const text = (choice: Choice) => choice[english ? 2 : 1]
  const close = (focus = false) => { setOpen(false); if (focus) trigger.current?.focus() }
  useLayoutEffect(() => {
    if (!open) return
    const anchor = trigger.current!.getBoundingClientRect()
    const width = Math.min(Math.max(anchor.width, 260), window.innerWidth - 24)
    const height = menu.current?.getBoundingClientRect().height || 240
    setPosition({ width, left: Math.max(12, Math.min(anchor.left, window.innerWidth - width - 12)), top: Math.max(12, anchor.bottom + height + 8 > window.innerHeight ? anchor.top - height - 8 : anchor.bottom + 8) })
    menu.current?.querySelectorAll<HTMLButtonElement>('[role="option"]')[selected]?.focus()
  }, [open, selected])
  useEffect(() => {
    if (!open) return
    const outside = (event: PointerEvent) => { if (!menu.current?.contains(event.target as Node) && !trigger.current?.contains(event.target as Node)) close() }
    const moved = (event: Event) => { if (!menu.current?.contains(event.target as Node)) close() }
    document.addEventListener('pointerdown', outside)
    window.addEventListener('resize', moved)
    window.addEventListener('scroll', moved, true)
    return () => { document.removeEventListener('pointerdown', outside); window.removeEventListener('resize', moved); window.removeEventListener('scroll', moved, true) }
  }, [open])
  useEffect(() => { if (disabled) setOpen(false) }, [disabled])
  return <div className="wave-select"><span id={`${id}-label`} className="wave-select-label">{label}</span>
    <button ref={trigger} type="button" className="wave-select-trigger" role="combobox" aria-labelledby={`${id}-label`} aria-expanded={open} aria-controls={open ? id : undefined} aria-haspopup="listbox" disabled={disabled} onClick={() => setOpen(!open)} onKeyDown={event => {
      if (['ArrowDown', 'ArrowUp', 'Enter', ' '].includes(event.key)) { event.preventDefault(); setOpen(true) }
      if (event.key === 'Escape') close()
    }}><span>{text(choices[selected])}</span><ChevronDown size={15} className={open ? 'is-open' : ''} /></button>
    {open && createPortal(<div id={id} ref={menu} role="listbox" aria-labelledby={`${id}-label`} className="wave-select-menu" style={position} onKeyDown={event => {
      const options = [...menu.current!.querySelectorAll<HTMLButtonElement>('[role="option"]')]
      const index = options.indexOf(document.activeElement as HTMLButtonElement)
      if (event.key === 'Escape') { event.preventDefault(); close(true) }
      else if (event.key === 'Tab') close(true)
      else if (['ArrowDown','ArrowUp','Home','End'].includes(event.key)) {
        event.preventDefault()
        const next = event.key === 'Home' ? 0 : event.key === 'End' ? options.length-1 : (index + (event.key === 'ArrowDown' ? 1 : -1) + options.length) % options.length
        options[next]?.focus()
      }
    }}>{choices.map(choice => <button key={choice[0]} type="button" role="option" aria-selected={choice[0] === value} tabIndex={-1} onClick={() => { close(true); onChange(choice[0]) }}><span>{text(choice)}</span>{choice[0] === value && <Check size={15} />}</button>)}</div>, document.body)}
  </div>
}
