import type { Settings } from './types'

export const themeFields = ['theme', 'accent_rgb', 'panel_rgb', 'panel_opacity', 'panel_blur', 'heading_opacity', 'text_rgb', 'muted_text_rgb', 'interface_text_scale', 'interface_scale', 'background_opacity', 'background_dim', 'background_blur', 'background_overlay', 'lyrics_scale', 'lyrics_blur_past', 'reduced_motion'] as const
export type ThemePreset = { version: 1; name: string; values: Partial<Pick<Settings, typeof themeFields[number]>> }
export const captureTheme = (settings: Settings, name: string): ThemePreset => ({ version: 1, name, values: Object.fromEntries(themeFields.map(key => [key, settings[key]])) })
const base = { panel_rgb: null, panel_opacity: .85, panel_blur: 12, heading_opacity: null, text_rgb: null, muted_text_rgb: null }
export const builtInThemes: ThemePreset[] = [
  { version: 1, name: 'Midnight', values: { ...base, theme: 'Dark', accent_rgb: [255, 85, 25] } },
  { version: 1, name: 'Daylight', values: { ...base, theme: 'Light', accent_rgb: [30, 100, 220] } },
  { version: 1, name: 'Lavender', values: { ...base, theme: 'Dark', accent_rgb: [184, 156, 255], panel_rgb: [23, 20, 34], text_rgb: [243, 239, 255], muted_text_rgb: [187, 180, 209] } },
  { version: 1, name: 'Ocean', values: { ...base, theme: 'Dark', accent_rgb: [93, 188, 238], panel_rgb: [17, 29, 40] } },
  { version: 1, name: 'Forest', values: { ...base, theme: 'Dark', accent_rgb: [121, 199, 158], panel_rgb: [20, 32, 28] } },
  { version: 1, name: 'Rose', values: { ...base, theme: 'Dark', accent_rgb: [236, 148, 177], panel_rgb: [35, 23, 30] } },
  { version: 1, name: 'Sand', values: { ...base, theme: 'Light', accent_rgb: [157, 101, 42], panel_rgb: [244, 237, 222] } },
  { version: 1, name: 'Graphite', values: { ...base, theme: 'Dark', accent_rgb: [195, 201, 211], panel_rgb: [26, 28, 32], panel_blur: 0 } },
]

export const themeIsActive = (settings: Settings, preset: ThemePreset) => Object.entries(preset.values).every(([key, value]) => {
  const current = settings[key as keyof Settings]
  return typeof value === 'number' && typeof current === 'number' ? Math.abs(current - value) < .00001 : JSON.stringify(current) === JSON.stringify(value)
})

export function validateTheme(input: unknown): ThemePreset {
  if (!input || typeof input !== 'object') throw new Error('Invalid theme file')
  const preset = input as ThemePreset
  if (Object.keys(preset).some(key => !['version', 'name', 'values'].includes(key)) || preset.version !== 1 || typeof preset.name !== 'string' || !preset.name.trim() || [...preset.name].length > 64 || !preset.values || typeof preset.values !== 'object' || Array.isArray(preset.values)) throw new Error('Invalid theme file')
  const bounds: Record<string, [number, number]> = { panel_opacity: [0, 1], heading_opacity: [0, 1], background_overlay: [0, 1], background_opacity: [0, .7], background_dim: [0, .85], panel_blur: [0, 40], background_blur: [0, 50], interface_text_scale: [.9, 1.25], interface_scale: [.9, 1.15], lyrics_scale: [.8, 1.5] }
  if (!Object.keys(preset.values).length) throw new Error('Empty theme')
  for (const [key, value] of Object.entries(preset.values)) {
    if (!(themeFields as readonly string[]).includes(key)) throw new Error(`Unexpected theme field: ${key}`)
    if (key.endsWith('_rgb')) {
      if (value === null && key !== 'accent_rgb') continue
      if (!Array.isArray(value) || value.length !== 3 || value.some(channel => !Number.isInteger(channel) || channel < 0 || channel > 255)) throw new Error('Invalid theme colour')
    } else if (key === 'theme') {
      if (!['Dark', 'Light', 'System'].includes(String(value))) throw new Error('Invalid theme mode')
    } else if (key === 'reduced_motion' || key === 'lyrics_blur_past') {
      if (typeof value !== 'boolean') throw new Error('Invalid theme toggle')
    } else {
      if (key === 'heading_opacity' && value === null) continue
      const [min, max] = bounds[key]
      if (typeof value !== 'number' || !Number.isFinite(value) || value < min || value > max || (key.endsWith('_blur') && !Number.isInteger(value))) throw new Error('Theme value out of range')
    }
  }
  return preset
}

// Recovered settings can contain damaged entries; preserve the valid themes.
export function savedThemes(input: unknown): ThemePreset[] {
  if (!Array.isArray(input)) return []
  const presets: ThemePreset[] = []
  const names = new Set<string>()
  for (const entry of input) {
    try {
      const preset = validateTheme(entry)
      if (!names.has(preset.name)) { presets.push(preset); names.add(preset.name) }
    } catch { /* Ignore only the damaged entry. */ }
    if (presets.length === 20) break
  }
  return presets
}
