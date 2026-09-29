import { useRef } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'

export const musicGenres = [
  'Electronic', 'Hip-hop & Rap', 'Ambient', 'Alternative Rock', 'Lo-fi',
  'House', 'Phonk', 'R&B', 'Trap', 'Jazz', 'Techno', 'Indie', 'Soul',
  'Drum & Bass', 'Hyperpop', 'K-pop', 'Pop', 'Rock', 'Metal', 'Classical', 'Downtempo',
] as const

export function GenreCarousel({ selected, onSelect, english = false, showAll = false }: { selected?: string; onSelect: (genre: string) => void; english?: boolean; showAll?: boolean }) {
  const strip = useRef<HTMLDivElement>(null)
  const scroll = (direction: number) => strip.current?.scrollBy({ left: direction * 360, behavior: 'smooth' })
  return <div className="genre-carousel" aria-label={english ? 'Music genres' : 'Музыкальные жанры'}>
    <button className="genre-arrow" aria-label={english ? 'Previous genres' : 'Предыдущие жанры'} onClick={() => scroll(-1)}><ChevronLeft size={17} /></button>
    <div className="genre-carousel-strip" ref={strip} role="group">{showAll && <button className={!selected ? 'active' : ''} onClick={() => onSelect('')}>{english ? 'All genres' : 'Все жанры'}</button>}{musicGenres.map((genre, index) => <button key={genre} className={selected === genre ? 'active' : ''} onClick={() => onSelect(genre)}><i style={{ '--genre-hue': `${(index * 43 + 16) % 360}` } as React.CSSProperties} aria-hidden="true" />{genre}</button>)}</div>
    <button className="genre-arrow" aria-label={english ? 'Next genres' : 'Следующие жанры'} onClick={() => scroll(1)}><ChevronRight size={17} /></button>
  </div>
}
