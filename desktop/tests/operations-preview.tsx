// Isolated local UI fixture; never loaded by the application entry point.
import React from 'react'
import { createRoot } from 'react-dom/client'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { ServerOperations, type OperationsSnapshot } from '../src/ServerOperations'
import { api } from '../src/api'
import '../src/style.css'
import '../src/features.css'
import '../src/theme.css'

const english=new URLSearchParams(location.search).get('lang')==='en'
const summary={requests:100,errors:2,limited:1,p95_ms:183.5}
let snapshot:OperationsSnapshot={since:0,window_seconds:300,http:summary,upstream:{...summary,retries:3},resources:{free_disk_bytes:8*1024**3,cpu_count:2,load_average:[.4,.3,.2]},traffic:{month:'fixture',inbound_bytes:1024,outbound_bytes:80*1024**3,upstream_bytes:2*1024**3,total_bytes:82*1024**3,limit_bytes:100*1024**3,state:'warning'},settings:{monthly_limit_bytes:100*1024**3,alert_percent:80,backup_interval_hours:6},backup:{state:'ok',last_success:1791241200,error:null,retained:8},cache:{cache_hits:90,cache_misses:10,active_downloads:1},grouping:{hits:40,grouped:12,inflight:1},lanes:{audio:summary},incidents:[]}
api.serverOperations=async(_url,settings)=>{if(settings)snapshot={...snapshot,settings:{...snapshot.settings,...settings}};return snapshot}
api.serverIncident=async()=>({incidents:[]})
const client=new QueryClient()
document.body.style.overflow='auto'
createRoot(document.getElementById('root')!).render(<QueryClientProvider client={client}><main style={{width:'min(920px, calc(100% - 40px))',margin:'30px auto'}}><p>{english?'Test data — no real server':'Тестовые данные — не реальный сервер'}</p><ServerOperations english={english} serverUrl="https://fixture.invalid" accountId={1}/></main></QueryClientProvider>)
