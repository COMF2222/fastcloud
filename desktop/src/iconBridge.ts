import { invoke } from '@tauri-apps/api/core'
let previewIcon = 9
export async function iconRequest(id?: number) {
  if ('__TAURI_INTERNALS__' in window) return invoke<{selected: number; supported: boolean; note?: string | null}>('application_icon', { id: id ?? null })
  if (id !== undefined) previewIcon = id
  return { selected: previewIcon, supported: false }
}

export const supportsSystemIcon = '__TAURI_INTERNALS__' in window
