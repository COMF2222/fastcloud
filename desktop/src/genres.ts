import type { Playlist, Track } from './types'

const aliases: Record<string, string> = {
  'k pop': 'K-pop', kpop: 'K-pop', 'korean pop': 'K-pop',
  hyperpop: 'Hyperpop', 'hyper pop': 'Hyperpop',
  'hip hop': 'Hip-hop & Rap', 'hip hop rap': 'Hip-hop & Rap', hiphop: 'Hip-hop & Rap', rap: 'Hip-hop & Rap',
  'r b': 'R&B', 'rhythm and blues': 'R&B',
  'drum bass': 'Drum & Bass', dnb: 'Drum & Bass',
  lofi: 'Lo-fi', 'lo fi': 'Lo-fi',
  'alt rock': 'Alternative Rock', 'alternative rock': 'Alternative Rock',
  'deep house': 'Deep House', 'dark techno': 'Dark Techno',
}

const ignored = new Set(['', 'all music', 'music', 'other', 'none', 'unknown', 'n a'])
const knownGenres = new Set([
  ...Object.values(aliases), 'Electronic', 'Ambient', 'House', 'Phonk', 'Trap', 'Jazz',
  'Techno', 'Indie', 'Soul', 'Pop', 'Rock', 'Metal', 'Classical', 'Downtempo',
  'Shoegaze', 'Dance', 'Funk', 'Brazilian funk', 'Hardcore', 'Synthwave',
].map(value => genreKey(value)))

export function genreKey(value: string | null | undefined): string {
  const key = (value || '').normalize('NFKC').toLocaleLowerCase()
    .replace(/&/g, ' and ').replace(/[^\p{L}\p{N}]+/gu, ' ').trim().replace(/\s+/g, ' ')
    .replace(/ and /g, ' ')
  return ignored.has(key) ? '' : (aliases[key] || key)
}

export function sameGenre(left: string | null | undefined, right: string | null | undefined): boolean {
  const wanted = genreKey(right)
  return !!wanted && (left || '').split(/\s*[,/|;]\s*/).some(part => genreKey(part) === wanted)
}

// SoundCloud tags may contain quoted phrases. Treat only a complete tag as a genre.
function tags(value: string | null | undefined): string[] {
  return [...(value || '').matchAll(/"([^"]+)"|([^\s]+)/g)].map(match => match[1] || match[2])
}

function taggedGenre(value: string | null | undefined): string | null {
  const matches = [...new Map(tags(value).filter(tag => knownGenres.has(genreKey(tag))).map(tag => [genreKey(tag), tag])).values()]
  return matches.length === 1 ? matches[0] : null
}

export function trackGenre(track: Track): string | null {
  const explicit = track.genre?.trim()
  if (genreKey(explicit)) return explicit!
  return taggedGenre(track.tag_list)
}

export function releaseGenres(release: Playlist, tracks: Track[] = release.tracks || []): string[] {
  const explicit = release.genre?.trim()
  const counts = new Map<string, { label: string; count: number }>()
  let classified = 0
  for (const track of tracks) {
    const label = trackGenre(track)
    const key = genreKey(label)
    if (!key) continue
    classified++
    const entry = counts.get(key) || { label: label!, count: 0 }
    entry.count++
    counts.set(key, entry)
  }
  const ranked = [...counts.entries()].sort((a, b) => b[1].count - a[1].count)
  const majority = ranked[0]
  const strongMajority = majority && ((classified >= 2 && majority[1].count / classified >= 0.75) || (classified === 1 && release.track_count === 1))
  if (strongMajority && explicit && !sameGenre(explicit, majority[1].label)) return [majority[1].label]
  if (genreKey(explicit)) return explicit!.split(/\s*[,/|;]\s*/).filter(part => genreKey(part))
  if (majority && majority[1].count / classified >= 0.6) return [majority[1].label]
  const tagged = taggedGenre(release.tag_list)
  return tagged ? [tagged] : []
}

export function releaseMatchesGenre(release: Playlist, genre: string): boolean {
  return releaseGenres(release).some(value => sameGenre(value, genre))
}
