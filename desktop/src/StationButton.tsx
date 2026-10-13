import { useState } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import { LoaderCircle, Radio } from 'lucide-react'
import { api } from './api'
import type { Track } from './types'

export function StationButton({ kind, tracks, load, english = false, disabled = false, iconOnly = false }: {
  kind: 'track' | 'collection' | 'artist'; tracks?: Track[]; load?: () => Promise<Track[]>; english?: boolean; disabled?: boolean; iconOnly?: boolean
}) {
  const queryClient = useQueryClient()
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const label = english ? { track: 'Track radio', collection: 'Collection radio', artist: 'Artist radio' }[kind]
    : { track: 'Волна по треку', collection: 'Волна по подборке', artist: 'Волна по артисту' }[kind]
  const start = async () => {
    if (busy) return
    setBusy(true); setError('')
    try {
      await api.station(load ? await load() : tracks || [], kind)
      await queryClient.invalidateQueries({ queryKey: ['player'] })
    } catch { setError(english ? 'Could not start radio. Try again.' : 'Не удалось запустить волну. Попробуй ещё раз.') }
    finally { setBusy(false) }
  }
  return <><button className={iconOnly ? 'now-playing-round' : 'secondary-button'} title={label} aria-label={label} disabled={disabled || busy} aria-busy={busy} onClick={() => void start()}>
    {busy ? <LoaderCircle className="spin" size={16} /> : <Radio size={16} />} {!iconOnly && label}
  </button>{error && <span className="error-text" role="status">{error}</span>}</>
}
