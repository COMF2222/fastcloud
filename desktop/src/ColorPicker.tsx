import { useEffect, useLayoutEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import { X } from 'lucide-react'
import { colorHex, colorRgb } from './theme'

type Hsv = { h: number; s: number; v: number }
const clamp = (value: number) => Math.max(0, Math.min(100, value))
function fromHex(hex: string): Hsv {
  const [r, g, b] = colorRgb(hex).map(value => value / 255)
  const max = Math.max(r, g, b), min = Math.min(r, g, b), delta = max - min
  let h = delta === 0 ? 0 : max === r ? ((g - b) / delta) % 6 : max === g ? (b - r) / delta + 2 : (r - g) / delta + 4
  h = (h * 60 + 360) % 360
  return { h, s: max === 0 ? 0 : delta / max * 100, v: max * 100 }
}
function toHex({ h, s, v }: Hsv) {
  const chroma = v / 100 * s / 100, x = chroma * (1 - Math.abs((h / 60) % 2 - 1)), m = v / 100 - chroma
  const channels = h < 60 ? [chroma, x, 0] : h < 120 ? [x, chroma, 0] : h < 180 ? [0, chroma, x] : h < 240 ? [0, x, chroma] : h < 300 ? [x, 0, chroma] : [chroma, 0, x]
  return colorHex(channels.map(value => (value + m) * 255))
}

const presets = ['#ff5519', '#ee8474', '#e8b86d', '#9cbb91', '#78becb', '#84a4ed', '#b19bdf', '#eef0f6', '#aeb5c6', '#182235', '#12141d', '#000000']

export function ColorPicker({ value, label, english, onChange, onCommit, id }: { value: string; label: string; english: boolean; onChange: (hex: string) => void; onCommit: (hex: string) => void; id?: string }) {
  const [open, setOpen] = useState(false)
  const [hsv, setHsv] = useState(() => fromHex(value))
  const hsvRef = useRef(hsv)
  const [hexDraft, setHexDraft] = useState(value)
  const [position, setPosition] = useState({ left: 12, top: 12 })
  const anchor = useRef<HTMLButtonElement>(null)
  const panel = useRef<HTMLDivElement>(null)
  const palette = useRef<HTMLDivElement>(null)
  const dragging = useRef<number | null>(null)
  const commitRef = useRef(onCommit)
  commitRef.current = onCommit
  const t = (ru: string, en: string) => english ? en : ru
  useEffect(() => {
    const next = fromHex(value)
    if (next.s === 0) next.h = hsvRef.current.h
    hsvRef.current = next; setHsv(next); setHexDraft(value)
  }, [value])
  const change = (next: Hsv) => {
    hsvRef.current = next; setHsv(next)
    const hex = toHex(next)
    setHexDraft(hex); onChange(hex)
  }
  const close = (restoreFocus = false) => {
    commitRef.current(toHex(hsvRef.current))
    setOpen(false)
    if (restoreFocus) anchor.current?.focus()
  }
  useLayoutEffect(() => {
    if (!open) return
    const place = () => {
      const button = anchor.current, popup = panel.current
      if (!button || !popup) return
      const rect = button.getBoundingClientRect(), height = popup.getBoundingClientRect().height, width = popup.getBoundingClientRect().width
      const below = rect.bottom + 8, above = rect.top - height - 8
      setPosition({ left: Math.max(12, Math.min(rect.left, window.innerWidth - width - 12)), top: Math.max(12, Math.min(below + height <= window.innerHeight - 12 ? below : above, window.innerHeight - height - 12)) })
    }
    place(); palette.current?.focus({ preventScroll: true })
    const observer = new ResizeObserver(place)
    if (panel.current) observer.observe(panel.current)
    const outside = (event: Event) => {
      const target = event.target as Node
      if (!panel.current?.contains(target) && !anchor.current?.contains(target)) close()
    }
    const key = (event: KeyboardEvent) => { if (event.key === 'Escape') { event.preventDefault(); event.stopPropagation(); close(true) } }
    document.addEventListener('pointerdown', outside, true)
    document.addEventListener('focusin', outside)
    document.addEventListener('keydown', key, true)
    document.addEventListener('scroll', place, true)
    window.addEventListener('resize', place)
    return () => {
      observer.disconnect()
      document.removeEventListener('pointerdown', outside, true)
      document.removeEventListener('focusin', outside)
      document.removeEventListener('keydown', key, true)
      document.removeEventListener('scroll', place, true)
      window.removeEventListener('resize', place)
    }
  }, [open])
  const pointer = (event: React.PointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect()
    change({ h: hsvRef.current.h, s: clamp((event.clientX - rect.left) / rect.width * 100), v: clamp(100 - (event.clientY - rect.top) / rect.height * 100) })
  }
  return <>
    <button id={id} ref={anchor} type="button" className="color-picker-trigger" aria-label={`${t('Выбрать цвет', 'Choose colour')}: ${label}`} aria-haspopup="dialog" aria-expanded={open} onClick={() => open ? close() : setOpen(true)}><span className="color-picker-swatch" style={{ backgroundColor: value }} /><span>{value.toUpperCase()}</span></button>
    {open && createPortal(<div ref={panel} className="color-picker-popover" role="dialog" aria-label={`${t('Выбор цвета', 'Colour picker')}: ${label}`} style={position}>
      <div className="color-picker-heading"><strong>{label}</strong><button className="icon-button" aria-label={t('Закрыть выбор цвета', 'Close colour picker')} onClick={() => close(true)}><X size={17} /></button></div>
      <div ref={palette} className="color-picker-palette" role="slider" tabIndex={0} aria-label={t('Насыщенность и яркость', 'Saturation and brightness')} aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(hsv.s)} aria-valuetext={`${t('Насыщенность', 'Saturation')} ${Math.round(hsv.s)}%, ${t('яркость', 'brightness')} ${Math.round(hsv.v)}%`} style={{ backgroundColor: `hsl(${hsv.h} 100% 50%)` }} onPointerDown={event => { if (event.button !== 0) return; event.preventDefault(); event.currentTarget.focus(); dragging.current = event.pointerId; event.currentTarget.setPointerCapture(event.pointerId); pointer(event) }} onPointerMove={event => { if (dragging.current === event.pointerId) pointer(event) }} onPointerUp={event => { if (dragging.current !== event.pointerId) return; pointer(event); dragging.current = null; commitRef.current(toHex(hsvRef.current)) }} onPointerCancel={() => { dragging.current = null; commitRef.current(toHex(hsvRef.current)) }} onKeyDown={event => {
        if (!['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return
        event.preventDefault()
        const step = event.shiftKey ? 10 : 2, next = { ...hsvRef.current }
        if (event.key === 'ArrowLeft') next.s = clamp(next.s - step)
        if (event.key === 'ArrowRight') next.s = clamp(next.s + step)
        if (event.key === 'ArrowUp') next.v = clamp(next.v + step)
        if (event.key === 'ArrowDown') next.v = clamp(next.v - step)
        change(next)
      }} onKeyUp={() => commitRef.current(toHex(hsvRef.current))}><span className="color-picker-cursor" style={{ left: `${hsv.s}%`, top: `${100 - hsv.v}%` }} /></div>
      <label className="color-picker-hue-label"><span>{t('Оттенок', 'Hue')}</span><input className="color-picker-hue" type="range" min={0} max={359} value={hsv.h} aria-label={t('Оттенок', 'Hue')} onChange={event => change({ ...hsvRef.current, h: Number(event.target.value) })} onPointerUp={() => commitRef.current(toHex(hsvRef.current))} onKeyUp={() => commitRef.current(toHex(hsvRef.current))} /></label>
      <div className="color-picker-presets">{presets.map(hex => <button type="button" key={hex} aria-label={`${t('Цвет', 'Colour')} ${hex}`} title={hex.toUpperCase()} style={{ backgroundColor: hex }} onClick={() => { change(fromHex(hex)); commitRef.current(hex) }} />)}</div>
      <label className="color-picker-hex-label"><span>HEX</span><input type="text" aria-label={`${label} HEX`} value={hexDraft} maxLength={7} spellCheck={false} onChange={event => {
        const hex = event.target.value; setHexDraft(hex)
        if (/^#[0-9a-f]{6}$/i.test(hex)) { hsvRef.current = fromHex(hex); setHsv(hsvRef.current); onChange(hex) }
      }} onBlur={() => { setHexDraft(toHex(hsvRef.current)); commitRef.current(toHex(hsvRef.current)) }} onKeyDown={event => { if (event.key === 'Enter') { setHexDraft(toHex(hsvRef.current)); commitRef.current(toHex(hsvRef.current)) } }} /><span className="color-picker-swatch" style={{ backgroundColor: value }} /></label>
    </div>, document.body)}
  </>
}
