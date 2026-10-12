import { useEffect, useState } from 'react'
import { daypart, waveGreeting } from './waveGreeting'

export function WaveGreeting({ english }: { english: boolean }) {
  const [time, setTime] = useState(() => ({ hour: new Date().getHours(), variation: Math.floor(Math.random() * 4) }))
  useEffect(() => {
    const update = () => {
      const hour = new Date().getHours()
      setTime(previous => daypart(previous.hour) === daypart(hour) ? previous : { hour, variation: Math.floor(Math.random() * 4) })
    }
    const timer = window.setInterval(update, 30000)
    window.addEventListener('focus', update)
    return () => { window.clearInterval(timer); window.removeEventListener('focus', update) }
  }, [])
  return <h2>{waveGreeting(time.hour, english, time.variation)}</h2>
}
