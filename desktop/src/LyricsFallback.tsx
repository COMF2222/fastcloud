import { api } from './api'
import type { LyricsRecord } from './types'

export function LyricsFallback({ english }: { english: boolean }) {
  return <div className="lyrics-fallback">
    <p>{english ? 'No matching lyrics found.' : 'Подходящий текст не найден.'}</p>
  </div>
}

export function LyricsSource({ record, english }: { record: LyricsRecord; english: boolean }) {
  const label = record.source || 'LRCLIB'
  return <>{english ? 'Source' : 'Источник'}: {record.sourceUrl
    ? <button className="lyrics-source-link" onClick={() => void api.openLyricsSource(record.sourceUrl!).catch(() => {})}>{label}</button>
    : <strong>{label}</strong>}</>
}
