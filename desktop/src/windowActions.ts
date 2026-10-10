export type AppWindow = {
  minimize(): Promise<void>
  close(): Promise<void>
  isFullscreen(): Promise<boolean>
  setFullscreen(value: boolean): Promise<void>
  toggleMaximize(): Promise<void>
}

export async function windowAction(window: AppWindow, kind: 'minimize' | 'maximize' | 'close', mini = false) {
  if (kind === 'minimize') await window.minimize()
  else if (kind === 'close') await window.close()
  else if (!mini) {
    if (await window.isFullscreen()) await window.setFullscreen(false)
    else await window.toggleMaximize()
  }
}
