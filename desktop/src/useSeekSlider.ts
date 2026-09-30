import { useEffect, useRef, useState, type ChangeEvent, type FocusEvent, type KeyboardEvent, type PointerEvent } from 'react'

const seekKeys = new Set(['ArrowLeft', 'ArrowRight', 'Home', 'End', 'PageUp', 'PageDown'])

/** Show a draft position only while the user is actively moving the slider. */
export function useSeekSlider(trackKey: string, actualPosition: number, onSeek: (ms: number) => void) {
  const [draft, setDraft] = useState<{ trackKey: string; value: number } | null>(null)
  const interacting = useRef(false)
  const lastCommit = useRef<{ trackKey: string; value: number } | null>(null)

  useEffect(() => {
    interacting.current = false
    lastCommit.current = null
    setDraft(null)
  }, [trackKey])

  const commit = (value: number) => {
    interacting.current = false
    setDraft(null)
    const next = Math.max(0, Math.round(value))
    lastCommit.current = { trackKey, value: next }
    onSeek(next)
  }

  return {
    position: draft?.trackKey === trackKey ? draft.value : actualPosition,
    onChange: (event: ChangeEvent<HTMLInputElement>) => {
      const value = Number(event.currentTarget.value)
      if (interacting.current) setDraft({ trackKey, value })
      else if (lastCommit.current?.trackKey !== trackKey || lastCommit.current.value !== value) commit(value)
    },
    onPointerDown: () => { interacting.current = true },
    onPointerUp: (event: PointerEvent<HTMLInputElement>) => commit(Number(event.currentTarget.value)),
    onPointerCancel: () => { interacting.current = false; setDraft(null) },
    onKeyDown: (event: KeyboardEvent<HTMLInputElement>) => {
      if (seekKeys.has(event.key)) interacting.current = true
    },
    onKeyUp: (event: KeyboardEvent<HTMLInputElement>) => {
      if (seekKeys.has(event.key)) commit(Number(event.currentTarget.value))
    },
    onBlur: (event: FocusEvent<HTMLInputElement>) => {
      if (interacting.current) commit(Number(event.currentTarget.value))
    },
  }
}
