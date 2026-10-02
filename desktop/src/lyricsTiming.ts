export type LyricLine = { time: number; text: string }

export function currentLyricLine(lines: LyricLine[], positionMs: number): number {
  let low = 0, high = lines.length
  while (low < high) {
    const middle = (low + high) >>> 1
    if (lines[middle].time <= positionMs) low = middle + 1
    else high = middle
  }
  return low - 1
}

export function parseLrc(value: string): LyricLine[] {
  const lines: LyricLine[] = []
  for (const row of value.split(/\r?\n/)) {
    const timestamps = [...row.matchAll(/\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g)]
    const text = row.replace(/\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g, '').trim()
    for (const match of timestamps) {
      const fraction = match[3] ? Number(match[3].padEnd(3, '0').slice(0, 3)) : 0
      lines.push({ time: (Number(match[1]) * 60 + Number(match[2])) * 1000 + fraction, text })
    }
  }
  return lines.sort((a, b) => a.time - b.time)
}

