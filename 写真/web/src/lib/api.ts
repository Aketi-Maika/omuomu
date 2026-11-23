// NAS の公開 API との通信。すべての通信にログイン状態（署名つきトークン）を付ける
import { logout, saveSession, store } from './store'
import type { AuditRow, Manifest, PhotoStat, ResponseRecord, Stats, User } from './types'

export class SessionExpired extends Error {}

/** 管理ツールの中で Google ログインなしに表示を確かめている */
export function isMock() {
  return store.localPreview
}

export function emptyStat(): PhotoStat {
  return { n: 0, people: {}, unknown: 0, left: 0, staff: 0 }
}

export function addTo(st: PhotoStat, r: Pick<ResponseRecord, 'people' | 'unknown' | 'left' | 'staff'>) {
  st.n += 1
  for (const p of new Set(r.people)) st.people[p] = (st.people[p] ?? 0) + 1
  if (r.unknown > 0) st.unknown += 1
  if (r.left > 0) st.left += 1
  if (r.staff > 0) st.staff += 1
}

export function apiUrl(path: string) {
  if (!store.apiBase) throw new Error('写真の配信先（NAS）がまだ設定されていません')
  return `${store.apiBase}${path}`
}

export async function call(path: string, init: RequestInit = {}): Promise<Response> {
  const headers = new Headers(init.headers)
  if (store.user?.session) headers.set('Authorization', `Bearer ${store.user.session}`)
  if (init.body && !headers.has('Content-Type')) headers.set('Content-Type', 'application/json')
  let res: Response
  try {
    res = await fetch(apiUrl(path), { ...init, headers, credentials: 'omit' })
  } catch {
    throw new Error('写真の配信先（NAS）に接続できません。しばらくしてからお試しください')
  }
  if (res.status === 401) {
    logout()
    throw new SessionExpired('もう一度ログインしてください')
  }
  if (!res.ok) {
    const j = await res.json().catch(() => ({}))
    throw new Error(j.error ?? `エラー (${res.status})`)
  }
  return res
}

async function callJson<T>(path: string, init?: RequestInit): Promise<T> {
  return (await (await call(path, init)).json()) as T
}

export async function login(idToken: string): Promise<User> {
  const r = await callJson<{ session: string; email: string; name: string; role: User['role']; exp: number }>('/pub/login', {
    method: 'POST',
    body: JSON.stringify({ idToken }),
  })
  return { email: r.email, name: r.name, role: r.role, exp: r.exp, session: r.session }
}

export async function signIn(user: User, localPreview = false) {
  store.user = user
  store.localPreview = localPreview
  await fetchManifest()
  saveSession()
}

export async function fetchManifest(): Promise<Manifest> {
  store.manifest = await callJson<Manifest>('/pub/manifest')
  return store.manifest
}

export async function fetchStats(): Promise<Stats> {
  if (!store.manifest) return {}
  try {
    store.stats = (await callJson<{ photos: Stats }>('/pub/stats')).photos ?? {}
    store.statsError = ''
  } catch (e) {
    if (e instanceof SessionExpired) throw e
    store.statsError = '集計を読み込めませんでした（回答の保存はできます）'
    console.warn(e)
  }
  store.statsLoaded = true
  return store.stats
}

export async function submitResponse(r: ResponseRecord): Promise<void> {
  await call('/pub/submit', {
    method: 'POST',
    body: JSON.stringify({ photoId: r.photoId, people: r.people, unknown: r.unknown, left: r.left, staff: r.staff, sessionId: r.sessionId }),
  })
  addTo((store.stats[r.photoId] ??= emptyStat()), r)
}

/** 操作ログ（失敗しても画面の操作は止めない） */
export function logEvent(event: string, detail: Record<string, unknown> = {}) {
  call('/pub/log', { method: 'POST', body: JSON.stringify({ event, detail }) }).catch((e) => console.warn('log failed', e))
}

export async function fetchAudit(limit = 300): Promise<AuditRow[]> {
  return (await callJson<{ rows: AuditRow[] }>(`/pub/audit?limit=${limit}`)).rows
}

/** 管理者: 選んだ写真を ZIP で受け取る（NAS 側で記録される） */
export async function downloadZip(ids: string[], label: string, people: string[]): Promise<Blob> {
  const res = await call('/pub/zip', { method: 'POST', body: JSON.stringify({ ids, label, people }) })
  return res.blob()
}

export function sessionId(): string {
  try {
    let s = sessionStorage.getItem('omuomu.sid')
    if (!s) {
      s = crypto.getRandomValues(new Uint32Array(2)).join('-')
      sessionStorage.setItem('omuomu.sid', s)
    }
    return s
  } catch {
    return 'nosession'
  }
}
