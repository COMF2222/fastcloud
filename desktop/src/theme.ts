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
  settings.heading_opacity ?? (settings.panel_rgb ? 0
    : settings.background_image ? Math.min(.97, .18 + (settings.background_overlay ?? .8) * .98) : 0)))

const mix = (color: readonly number[], other: readonly number[], amount: number) => color.map((channel, i) => Math.round(channel * (1 - amount) + other[i] * amount))
const ease = (value: number, start: number, end: number) => {
  const progress = Math.max(0, Math.min(1, (value - start) / (end - start)))
  return progress * progress * (3 - 2 * progress)
}
export const luminance = (color: readonly number[]) => color.map(channel => {
  const value = channel / 255
  return value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4
}).reduce((sum, value, index) => sum + value * [.2126, .7152, .0722][index], 0)
export const contrast = (a: readonly number[], b: readonly number[]) => (Math.max(luminance(a), luminance(b)) + .05) / (Math.min(luminance(a), luminance(b)) + .05)

export function panelPalette(color: readonly number[], opacity: number, light: boolean) {
  const alpha = Math.max(0, Math.min(1, opacity))
  const base = light ? [241, 242, 246] : [14, 16, 21]
  // Keep low-opacity glass colourful, then ease into a restrained opaque
  // material. Opacity still reaches 100%; it never exposes the wallpaper.
  const brightness = luminance(color)
  const toneAmount = Math.max(light ? 1 - ease(brightness,.45,.85) : ease(brightness,.025,.09), ease(Math.max(...color) - Math.min(...color),45,140))
  const material = mix(color, mix(light ? [245, 246, 249] : [18, 20, 29], color, light ? .07 : .16), toneAmount)
  const surface = mix(color, material, ease(alpha,.15,.8))
  const nav = mix(surface, light ? [232, 234, 240] : [0, 0, 0], light ? .12 : .18)
  const raised = mix(surface, [255, 255, 255], light ? .1 : .035)
  const backgrounds = [surface, nav, raised].map(value => mix(base, value, alpha))
  const background = backgrounds[0]
  const minimumContrast = (foreground: readonly number[]) => Math.min(...backgrounds.map(value => contrast(value, foreground)))
  const darkText = minimumContrast([0, 0, 0]) >= minimumContrast([255, 255, 255])
  let text = darkText ? [20, 23, 29] : [245, 246, 249]
  if (minimumContrast(text) < 4.5) text = darkText ? [0, 0, 0] : [255, 255, 255]
  let muted = text
  for (const amount of [.28, .2, .12]) {
    const candidate = mix(text, background, amount)
    if (minimumContrast(candidate) >= 4.5) { muted = candidate; break }
  }
  let controlLine = text
  for (const amount of [.35, .45, .55, .7]) {
    const candidate = mix(background, text, amount)
    if (minimumContrast(candidate) >= 3) { controlLine = candidate; break }
  }
  return { surface, background, backgrounds, text, muted, controlLine, nav, raised, alpha }
}

export function readableAccent(accent: readonly number[], background: readonly number[], text: readonly number[]) {
  for (const amount of [0, .25, .5, .75, 1]) {
    const candidate = mix(accent, text, amount)
    if (contrast(background, candidate) >= 4.5) return candidate
  }
  return [...text]
}

// Floating controls must cover the page behind them, including at 0% panel
// opacity. Calculate their contrast against their own solid material.
export function popupPalette(settings: Pick<Settings, 'panel_rgb'>, light: boolean) {
  return panelPalette(settings.panel_rgb ?? (light ? [247, 248, 250] : [26, 29, 37]), 1, light)
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
  const light = root.dataset.theme === 'light'
  const palette = settings.panel_rgb ? panelPalette(settings.panel_rgb, settings.panel_opacity ?? .85, light) : null
  const headingPalette = settings.panel_rgb ? panelPalette(settings.panel_rgb, headingSurfaceOpacity(settings), light) : null
  const popup = popupPalette(settings, light)
  property('--popup-fill', rgb(popup.raised))
  property('--popup-card', rgb(popup.surface))
  property('--popup-text', rgb(settings.text_rgb ?? popup.text))
  property('--popup-muted', rgb(settings.muted_text_rgb ?? popup.muted))
  property('--popup-line', rgb(popup.controlLine))
  property('--popup-accent', rgb(readableAccent(settings.accent_rgb, popup.raised, popup.text)))
  property('--panel-fill', palette ? `rgb(${palette.surface.join(' ')} / ${palette.alpha})` : null)
  property('--panel-nav-fill', palette ? `rgb(${palette.nav.join(' ')} / ${palette.alpha})` : null)
  property('--panel-raised-fill', palette ? `rgb(${palette.raised.join(' ')} / ${palette.alpha})` : null)
  property('--panel-text', palette ? rgb(settings.text_rgb ?? palette.text) : null)
  property('--panel-muted', palette ? rgb(settings.muted_text_rgb ?? palette.muted) : null)
  property('--panel-line', palette ? `rgb(${palette.text.join(' ')} / .1)` : null)
  property('--panel-control-line', palette ? rgb(palette.controlLine) : null)
  property('--panel-accent', palette ? rgb(readableAccent(settings.accent_rgb, palette.backgrounds[light ? 0 : 2], palette.text)) : null)
  property('--page-text', rgb(settings.text_rgb ?? (light ? [32, 36, 49] : [241, 241, 244])))
  property('--page-muted', rgb(settings.muted_text_rgb ?? (light ? [98, 105, 121] : [146, 149, 161])))
  root.dataset.transparentHeadings = String(headingSurfaceOpacity(settings) < .5)
  property('--panel-blur', `${settings.panel_blur ?? 12}px`)
  property('--heading-opacity', settings.heading_opacity == null ? null : String(headingSurfaceOpacity(settings)))
  property('--user-heading-rgb', headingPalette ? headingPalette.surface.join(' ') : null)
  property('--heading-text', rgb(settings.text_rgb ?? headingPalette?.text ?? (light ? [32, 36, 49] : [241, 241, 244])))
  property('--heading-muted', rgb(settings.muted_text_rgb ?? headingPalette?.muted ?? (light ? [98, 105, 121] : [146, 149, 161])))
  property('--interface-text-scale', String(interfaceScale(settings, 'interface_text_scale')))
  property('--interface-scale', String(interfaceScale(settings, 'interface_scale')))
  property('--layout-scale', String(interfaceLayoutScale(settings)))
  property('--navigation-scale', String(Math.max(interfaceScale(settings, 'interface_scale'), Math.min(interfaceScale(settings, 'interface_text_scale'), 1.2))))
}
