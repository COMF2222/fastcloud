import React from 'react'
import ReactDOM from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import App from './App'
import { WindowControls } from './WindowControls'
import { UpdateProvider } from './Updater'
import { onLibraryUpdate } from './libraryCache'
import './style.css'
import './features.css'
import './theme.css'

const queryClient = new QueryClient({ defaultOptions: { queries: { retry: 1, staleTime: 800, refetchOnWindowFocus: false } } })
onLibraryUpdate((key, value, accountChanged) => {
  if (accountChanged) queryClient.removeQueries({ predicate: query => ['server-session', 'my-profile', 'tracks', 'playlists', 'approval-users', 'approval-settings', 'approval-media', 'server-operations', 'personal-collections', 'listening-statistics', 'smart-playlist'].includes(String(query.queryKey[0])) })
  const trackView = key.startsWith('tracks:') ? key.slice(7) : null
  const queryKey = key === 'profile' ? ['my-profile'] : trackView
    ? ['likes', 'discover'].includes(trackView) ? ['tracks', trackView, undefined, undefined] : ['tracks', trackView]
    : ['playlists', key.slice(10)]
  queryClient.setQueryData(queryKey, value)
})
ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode><QueryClientProvider client={queryClient}><UpdateProvider><App /><WindowControls /></UpdateProvider></QueryClientProvider></React.StrictMode>,
)
