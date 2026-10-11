import type { CSSProperties } from 'react'

// Render the original animation directly, without an intermediate background
// texture or a zero-radius filter that still creates a compositing surface.
export function Wallpaper({ url, blur = 0, className = '' }: { url: string; blur?: number; className?: string }) {
  const radius = Math.max(0, Math.min(40, blur))
  return <div className={`wallpaper-layer ${className}`} aria-hidden="true" style={{ '--wallpaper-blur': `${radius}px` } as CSSProperties}>
    <img className="wallpaper-image" src={url} alt="" draggable={false} loading="eager" style={{ filter: radius > 0 ? `blur(${radius}px)` : 'none' }} />
  </div>
}
