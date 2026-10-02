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
  property('--panel-blur', `${settings.panel_blur ?? 12}px`)
  property('--heading-opacity', settings.heading_opacity == null ? null : String(headingSurfaceOpacity(settings)))
  property('--user-heading-rgb', settings.panel_rgb ? settings.panel_rgb.join(' ') : null)
  property('--interface-text-scale', String(interfaceScale(settings, 'interface_text_scale')))
  property('--interface-scale', String(interfaceScale(settings, 'interface_scale')))
  property('--layout-scale', String(interfaceLayoutScale(settings)))
  property('--navigation-scale', String(Math.max(interfaceScale(settings, 'interface_scale'), Math.min(interfaceScale(settings, 'interface_text_scale'), 1.2))))
}
