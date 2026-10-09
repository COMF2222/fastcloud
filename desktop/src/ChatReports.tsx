import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { api } from './api'
import { chatError, type ChatMessage, type ChatPerson } from './chatTypes'
import { useChatSession } from './ChatPages'
import { useApp } from './store'

type Report = { id: number; reporterId: number; peerId: number; reason: string; createdAt: number; reporter: ChatPerson | null; peer: ChatPerson | null; messages: ChatMessage[] }
// Mounted only inside the owner operations panel. The endpoint independently
// checks the owner role; ordinary conversation endpoints still require membership.
export function ChatReports({ english }: { english: boolean }) {
  const [open, setOpen] = useState(false)
  const session = useChatSession()
  const { data, isPending, error } = useQuery({ queryKey: ['chat-reports', session.id], queryFn: () => api.chat<{ reports: Report[] }>('reports'), enabled: open && session.ready, refetchInterval: open ? 30_000 : false, retry: 1 })
  const t = (ru: string, en: string) => english ? en : ru
  return <details onToggle={event => setOpen(event.currentTarget.open)}><summary>{t('Жалобы из чатов Fastcloud', 'Fastcloud chat reports')}</summary>
    {isPending && open && <p>{t('Загружаем жалобы…', 'Loading reports…')}</p>}{error && <p role="alert">{chatError(error, english)}</p>}
    {data?.reports.length === 0 && <p>{t('Жалоб пока нет.', 'No reports yet.')}</p>}
    {data?.reports.map(report => <article className="chat-report" key={report.id}><p><strong>{report.reporter?.username || report.reporterId}</strong> → <button className="text-button" onClick={() => useApp.getState().openArtist(report.peerId, report.peer?.username || String(report.peerId))}>{report.peer?.username || report.peerId}</button> · {report.reason === 'spam' ? t('Спам', 'Spam') : t('Оскорбления', 'Harassment')} · {new Date(report.createdAt).toLocaleString(english ? 'en-US' : 'ru-RU')}</p>
      {report.messages.map(message => <p key={message.id}><strong>{message.senderId === report.reporterId ? report.reporter?.username || report.reporterId : report.peer?.username || report.peerId}:</strong> {message.text}{message.attachment && <span> · {message.attachment.title}</span>}</p>)}
    </article>)}
  </details>
}
