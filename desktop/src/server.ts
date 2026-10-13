// Set at build time. The address is public in the installed application.
export const FASTCLOUD_SERVER_URL = (import.meta.env.VITE_FASTCLOUD_SERVER_URL || 'https://80.47.238.200').trim().replace(/\/+$/, '')
