import type { Settings } from './types'

export const themeDefaults = {
  panel_rgb: null, panel_opacity: .85, panel_blur: 12, heading_opacity: null,
  text_rgb: null, muted_text_rgb: null, interface_text_scale: 1, interface_scale: 1,
} as const

export const colorHex = (rgb: readonly number[]) => `#${rgb.map(value => Math.round(Math.max(0, Math.min(255, value))).toString(16).padStart(2, '0')).join('')}`
export const colorRgb = (hex: string) => [1, 3, 5].map(index => parseInt(hex.slice(index, index + 2), 16))
export const interfaceLimits = { interface_text_scale: [.9, 1.25], interface_scale: [.9, 1.15] } as const
export const interfaceScale = (settings: Settings | undefined, key: keyof typeof interfaceLimits) => Math.max(interfaceLimits[key][0], Math.min(interfaceLimits[key][1], settings?.[key] ?? 1))
export const interfaceLayoutScale = (settings?: Settings) => Math.max(interfaceScale(settings, 'interface_scale'), interfaceScale(settings, 'interface_text_scale'))
export const interfaceRowHeight = (settings?: Settings) => 68 * interfaceLayoutScale(settings)
export const headingSurfaceOpacity = (settings: Settings) => Math.max(0, Math.min(1,
  settings.heading_opacity ?? (settings.panel_rgb ? settings.panel_opacity ?? .85
    : settings.background_image ? Math.min(.97, .18 + (settings.background_overlay ?? .8) * .98) : 0)))

const mix = (color: readonly number[], other: readonly number[], amount: number) => color.map((channel, i) => Math.round(channel * (1 - amount) + other[i] * amount))
export const luminance = (color: readonly number[]) => color.map(channel => {
  const value = channel / 255
  return value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4
}).reduce((sum, value, index) => sum + value * [.2126, .7152, .0722][index], 0)
export const contrast = (a: readonly number[], b: readonly number[]) => (Math.max(luminance(a), luminance(b)) + .05) / (Math.min(luminance(a), luminance(b)) + .05)

export function panelPalette(color: readonly number[], opacity: number, light: boolean) {
  const alpha = Math.max(0, Math.min(1, opacity))
  const background = mix(light ? [241, 242, 246] : [14, 16, 21], color, alpha)
  const darkText = contrast(background, [0, 0, 0]) >= contrast(background, [255, 255, 255])
  let text = darkText ? [20, 23, 29] : [245, 246, 249]
  if (contrast(background, text) < 4.5) text = darkText ? [0, 0, 0] : [255, 255, 255]
  let muted = text
  for (const amount of [.28, .2, .12]) {
    const candidate = mix(text, background, amount)
    if (contrast(background, candidate) >= 4.5) { muted = candidate; break }
  }
  const lightSurface = luminance(color) > .3
  return { background, text, muted, nav: mix(color, [0, 0, 0], lightSurface ? .07 : .12),
    raised: mix(color, lightSurface ? [0, 0, 0] : [255, 255, 255], lightSurface ? .035 : .045), alpha }
}

export function readableAccent(accent: readonly number[], background: readonly number[], text: readonly number[]) {
  for (const amount of [0, .25, .5, .75, 1]) {
    const candidate = mix(accent, text, amount)
    if (contrast(background, candidate) >= 4.5) return candidate
  }
  return [...text]
}

// Shared by saved settings and live slider/colour previews. Wallpaper dimming and
// lyric size are deliberately independent from reading surfaces and UI text.
export function applyThemeCustomization(settings: Settings) {
  const root = document.documentElement
  const rgb = (value: number[]) => `rgb(${value.join(' ')})`
  const property = (name: string, value: string | null) => value === null
    ? root.style.removeProperty(name) : root.style.setProperty(name, value)
  root.dataset.customPanels = String(!!settings.panel_rgb)
  root.dataset.customText = String(!!settings.text_rgb)
  root.dataset.customMutedText = String(!!settings.muted_text_rgb)
  root.dataset.headingOpacity = String(settings.heading_opacity != null)
  property('--user-text', settings.text_rgb ? rgb(settings.text_rgb) : null)
  property('--user-text-muted', settings.muted_text_rgb ? rgb(settings.muted_text_rgb) : null)
  property('--panel-color', settings.panel_rgb ? rgb(settings.panel_rgb) : null)
  property('--panel-fill', settings.panel_rgb ? `rgb(${settings.panel_rgb.join(' ')} / ${settings.panel_opacity ?? .85})` : null)
  const light = root.dataset.theme === 'light'
  const palette = settings.panel_rgb ? panelPalette(settings.panel_rgb, settings.panel_opacity ?? .85, light) : null
  property('--panel-nav-fill', palette ? `rgb(${palette.nav.join(' ')} / ${palette.alpha})` : null)
  property('--panel-raised-fill', palette ? `rgb(${palette.raised.join(' ')} / ${palette.alpha})` : null)
  property('--panel-text', palette ? rgb(settings.text_rgb ?? palette.text) : null)
  property('--panel-muted', palette ? rgb(settings.muted_text_rgb ?? palette.muted) : null)
  property('--panel-line', palette ? `rgb(${palette.text.join(' ')} / .16)` : null)
  property('--panel-accent', palette ? rgb(readableAccent(settings.accent_rgb, palette.background, palette.text)) : null)
  property('--page-text', rgb(settings.text_rgb ?? (light ? [32, 36, 49] : [241, 241, 244])))
  property('--page-muted', rgb(settings.muted_text_rgb ?? (light ? [98, 105, 121] : [146, 149, 161])))
  root.dataset.transparentHeadings = String(headingSurfaceOpacity(settings) < .5)
  property('--panel-blur', `${settings.panel_blur ?? 12}px`)
  property('--heading-opacity', settings.heading_opacity == null ? null : String(headingSurfaceOpacity(settings)))
  property('--user-heading-rgb', settings.panel_rgb ? settings.panel_rgb.join(' ') : null)
  property('--interface-text-scale', String(interfaceScale(settings, 'interface_text_scale')))
  property('--interface-scale', String(interfaceScale(settings, 'interface_scale')))
  property('--layout-scale', String(interfaceLayoutScale(settings)))
  property('--navigation-scale', String(Math.max(interfaceScale(settings, 'interface_scale'), Math.min(interfaceScale(settings, 'interface_text_scale'), 1.2))))
}
