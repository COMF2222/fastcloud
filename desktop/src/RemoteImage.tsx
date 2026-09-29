import { useEffect, useRef, useState, type ImgHTMLAttributes } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from './api'
import { soundcloudImageAt, soundcloudImageVariant } from './types'

const secure = (value: string) => value.replace(/^http:\/\//i, 'https://')
const soundcloudCdn = (value: string) => {
  try {
    const url = new URL(value)
    return url.hostname === 'sndcdn.com' || url.hostname.endsWith('.sndcdn.com')
  } catch { return false }
}

export function RemoteImage({ src, previewSrc, fallback, pixels, alt = '', ...props }: ImgHTMLAttributes<HTMLImageElement> & { src: string; previewSrc?: string; fallback?: string; pixels?: number }) {
  const [broken, setBroken] = useState<string | null>(null)
  const element = useRef<HTMLImageElement>(null)
  const [nearViewport, setNearViewport] = useState(props.loading !== 'lazy')
  useEffect(() => {
    if (nearViewport || props.loading !== 'lazy') return
    if (!('IntersectionObserver' in window)) { setNearViewport(true); return }
    const observer = new IntersectionObserver(entries => {
      if (entries.some(entry => entry.isIntersecting)) { setNearViewport(true); observer.disconnect() }
    }, { rootMargin: '240px' })
    if (element.current) observer.observe(element.current)
    return () => observer.disconnect()
  }, [nearViewport, props.loading])
  const primary = secure(pixels ? soundcloudImageAt(src, pixels) : src)
  const preview = previewSrc ? secure(previewSrc) : null
  const proxied = !api.preview && soundcloudCdn(primary)
  const variants = proxied
    ? [...new Set([
      primary,
      secure(soundcloudImageVariant(src, 't1080x1080')),
      secure(soundcloudImageVariant(src, 't500x500')),
      secure(soundcloudImageVariant(src, 'original')),
      fallback ? secure(fallback) : null,
      secure(src),
    ].filter((value): value is string => !!value))]
    : [primary]
  const image = useQuery({
    queryKey: ['artwork-image-best', ...variants],
    enabled: proxied && nearViewport,
    staleTime: Infinity,
    gcTime: 15_000,
    retry: false,
    queryFn: async () => {
      let lastError: unknown
      for (const url of variants) {
        try { return await api.imageData(url) }
        catch (error) { lastError = error }
      }
      throw lastError
    },
  })
  const thumbnail = useQuery({
    queryKey: ['artwork-image-preview', preview],
    enabled: proxied && nearViewport && !!preview && preview !== primary && !image.data,
    staleTime: Infinity,
    gcTime: 15_000,
    retry: false,
    queryFn: () => api.imageData(preview!),
  })
  const candidates = proxied
    ? [image.data, thumbnail.data, image.isError ? secure(fallback || src) : null]
    : [primary]
  const visible = candidates.find(value => !!value && value !== broken)
  return <img {...props} ref={element} src={visible || undefined} alt={alt} onError={() => visible && setBroken(visible)} />
}
