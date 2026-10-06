import { useEffect, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { api } from './api'

type Summary = { requests: number; errors: number; limited: number; p95_ms: number | null }
type Incident = { id: number; status: string; ru: string; en: string; started: number; resolved: number | null }
export type OperationsSnapshot = {
  since: number; window_seconds: number; http: Summary; upstream: Summary & { retries: number }
  resources: { free_disk_bytes?: number; cpu_count?: number; load_average?: number[] }
  traffic: { month: string; inbound_bytes: number; outbound_bytes: number; upstream_bytes: number; total_bytes: number; limit_bytes: number; state: string }
  settings: { monthly_limit_bytes: number; alert_percent: number; backup_interval_hours: number }
  backup: { state: string; last_success: number | null; error: string | null; retained: number }
  cache: { cache_hits: number; cache_misses: number; active_downloads: number } | null
  grouping: { hits: number; grouped: number; inflight: number } | null
  lanes: Record<string, Summary>; incidents: Incident[]
}
const gib = (bytes: number) => `${(bytes / 1024 ** 3).toFixed(2)} GiB`

export function ServerOperations({ english, serverUrl, accountId }: { english: boolean; serverUrl: string; accountId?: number }) {
  const t = (ru: string, en: string) => english ? en : ru
  const client = useQueryClient()
  const key = ['server-operations', serverUrl, accountId]
  const { data, error } = useQuery({ queryKey: key, queryFn: () => api.serverOperations(serverUrl), refetchInterval: 10000, retry: false })
  const [limit, setLimit] = useState('0'), [percent, setPercent] = useState(80), [hours, setHours] = useState(6)
  const [editing, setEditing] = useState(false), [busy, setBusy] = useState(false), [message, setMessage] = useState('')
  const [ru, setRu] = useState(''), [en, setEn] = useState(''), [incidentStatus, setIncidentStatus] = useState('degraded')
  useEffect(() => {
    if (data && !editing) {
      setLimit(String(data.settings.monthly_limit_bytes / 1024 ** 3)); setPercent(data.settings.alert_percent); setHours(data.settings.backup_interval_hours)
    }
  }, [data?.settings.monthly_limit_bytes, data?.settings.alert_percent, data?.settings.backup_interval_hours, editing])
  const run = async (work: () => Promise<unknown>) => {
    setBusy(true); setMessage('')
    try { await work(); await client.invalidateQueries({ queryKey: key }); setMessage(t('Сохранено', 'Saved')) }
    catch { setMessage(t('Не удалось сохранить. Проверь подключение и версию сервера.', 'Could not save. Check your connection and server version.')) }
    finally { setBusy(false) }
  }
  if (error && !data) return <p role="status">{t('Мониторинг недоступен. Обнови бэкенд и проверь подключение.', 'Monitoring is unavailable. Update the backend and check the connection.')}</p>
  if (!data) return null
  const cacheRequests = (data.cache?.cache_hits || 0) + (data.cache?.cache_misses || 0)
  return <section className="server-operations"><h3>{t('Здоровье сервера', 'Server health')}</h3>
    <p className="muted">{t('Последние 5 минут · обновление каждые 10 секунд', 'Last 5 minutes · refreshed every 10 seconds')}</p>
    {error && <p role="status">{t('Показан последний полученный снимок. Новые данные временно недоступны.', 'Showing the last snapshot. Fresh data is temporarily unavailable.')}</p>}
    <div className="operations-metrics">
      <div><strong>{data.http.p95_ms == null ? '—' : `${data.http.p95_ms} ms`}</strong><small>{t('p95 ответа сервера', 'Server response p95')}</small></div>
      <div><strong>{data.upstream.errors} / {data.upstream.requests}</strong><small>{t('Ошибки источника / запросы', 'Upstream errors / requests')}</small></div>
      <div><strong>{data.upstream.limited}</strong><small>{t('Ограничения SoundCloud', 'SoundCloud rate limits')}</small></div>
      <div><strong>{cacheRequests ? `${Math.round((data.cache?.cache_hits || 0) / cacheRequests * 100)}%` : '—'}</strong><small>{t('Попадания в аудиокеш с запуска', 'Audio cache hits since startup')}</small></div>
      <div><strong>{data.resources.free_disk_bytes == null ? '—' : gib(data.resources.free_disk_bytes)}</strong><small>{t('Свободно на диске базы', 'Free database disk space')}</small></div>
      <div><strong>{data.resources.load_average?.[0]?.toFixed(2) ?? '—'}</strong><small>{t('Нагрузка за минуту', 'One-minute load average')} · {data.resources.cpu_count ?? '—'} CPU</small></div>
    </div>
    <p>{t('Сгруппировано одинаковых запросов', 'Duplicate requests grouped')}: {data.grouping?.grouped ?? '—'} · {t('Обложки из памяти', 'Artwork memory hits')}: {data.grouping?.hits ?? '—'} · {t('Повторные обращения к источнику', 'Upstream retries')}: {data.upstream.retries}</p>
    <h3>{t('Бюджет трафика', 'Traffic budget')} · {data.traffic.month} UTC</h3>
    <p>{gib(data.traffic.total_bytes)}{data.traffic.limit_bytes ? ` / ${gib(data.traffic.limit_bytes)}` : ''} · {t('Клиентам', 'To clients')}: {gib(data.traffic.outbound_bytes)} · {t('Обмен с источником', 'Upstream exchange')}: {gib(data.traffic.upstream_bytes)}</p>
    {['warning','exceeded'].includes(data.traffic.state) && <p className="operations-warning" role="alert">{data.traffic.state === 'exceeded' ? t('Заданный бюджет превышен.', 'Configured budget exceeded.') : t('Трафик приближается к заданному бюджету.', 'Traffic is approaching the configured budget.')}</p>}
    <p className="muted">{t('Учтены байты, прочитанные и отправленные приложением. TLS, VPN и другие процессы VPS не входят — это не точный счётчик провайдера. Бюджет предупреждает и не обрывает музыку.', 'Counts bytes read and sent by the application. TLS, VPN and other VPS processes are excluded; this is not the provider’s billing meter. The budget warns without stopping music.')}</p>
    <form className="operations-form" onSubmit={event => { event.preventDefault(); void run(async () => {
      const bytes = Math.round(Number(limit) * 1024 ** 3)
      if (!Number.isFinite(bytes) || bytes < 0 || bytes > 100 * 1024 ** 4 || !Number.isInteger(percent) || percent < 1 || percent > 100) throw new Error('Invalid budget')
      const next = await api.serverOperations(serverUrl,{ monthly_limit_bytes: bytes, alert_percent: percent, backup_interval_hours: hours })
      client.setQueryData(key,next); setEditing(false)
    }) }}>
      <label className="field-label">{t('Бюджет в GiB · 0 — выключен', 'Budget in GiB · 0 disables')}<input type="number" min="0" max="102400" step="any" value={limit} onChange={event => { setEditing(true); setLimit(event.target.value) }} /></label>
      <label className="field-label">{t('Предупреждать при %', 'Warn at %')}<input type="number" min="1" max="100" value={percent} onChange={event => { setEditing(true); setPercent(Number(event.target.value)) }} /></label>
      <label className="field-label">{t('Копия базы каждые', 'Database backup every')}<select value={hours} onChange={event => { setEditing(true); setHours(Number(event.target.value)) }}>{Array.from({ length:24 },(_,index)=>index+1).map(hours => <option key={hours} value={hours}>{hours} {t('ч', 'h')}</option>)}</select></label>
      <button className="secondary-button" disabled={busy || !editing}>{t('Сохранить', 'Save')}</button>
    </form>
    <p>{t('Резервная копия', 'Backup')}: {data.backup.state === 'error' ? t('Ошибка создания или проверки', 'Creation or verification failed') : data.backup.last_success ? new Date(data.backup.last_success * 1000).toLocaleString(english ? 'en-GB' : 'ru-RU') : t('Первая копия ещё не создана', 'First backup is pending')} · {data.backup.retained} {t('копий', 'snapshots')}</p>
    <p className="muted">{t('Копии хранятся в отдельном томе этого VPS. Для восстановления после потери сервера нужна копия вне VPS; команды переноса есть в документации бэкенда.', 'Snapshots live in a separate volume on this VPS. Recovering from loss of the server requires an offsite copy; export commands are in the backend documentation.')}</p>
    <details><summary>{t('Сообщение на публичной странице статуса', 'Public status announcement')}</summary>
      <p>{t('Этот текст увидят посетители сайта. Укажи проблему и действие пользователя, без личных данных и токенов.', 'Website visitors will see this text. Describe the problem and any user action, without personal data or tokens.')}</p>
      <form className="operations-form" onSubmit={event => { event.preventDefault(); void run(async () => { await api.serverIncident(serverUrl,{ status:incidentStatus,ru:ru.trim(),en:en.trim() }); setRu(''); setEn('') }) }}>
        <label className="field-label">{t('Состояние', 'Status')}<select value={incidentStatus} onChange={event => setIncidentStatus(event.target.value)}><option value="degraded">{t('Работает с перебоями', 'Degraded')}</option><option value="maintenance">{t('Обслуживание', 'Maintenance')}</option><option value="outage">{t('Недоступен', 'Outage')}</option></select></label>
        <label className="field-label">{t('Сообщение по-русски', 'Russian message')}<textarea value={ru} maxLength={500} onChange={event => setRu(event.target.value)} /></label>
        <label className="field-label">{t('Сообщение по-английски', 'English message')}<textarea value={en} maxLength={500} onChange={event => setEn(event.target.value)} /></label>
        <button className="secondary-button" disabled={busy || !ru.trim() || !en.trim()}>{t('Опубликовать сообщение', 'Publish announcement')}</button>
      </form>
      {data.incidents.filter(incident => !incident.resolved).map(incident => <div className="operations-incident" key={incident.id}><p>{english ? incident.en : incident.ru}</p><button className="secondary-button" disabled={busy} onClick={() => void run(() => api.serverIncident(serverUrl,{ resolve:incident.id }))}>{t('Отметить решённым', 'Mark resolved')}</button></div>)}
    </details>
    {message && <p role="status">{message}</p>}
  </section>
}
