// Apps Script（Google ログインの確認・回答の保存・操作ログ）との通信
// この PC での表示確認（localPreview）中は、回答をこのブラウザ内(localStorage)にだけ保存する
import { logout, store } from './store'
import type { AuditRow, PhotoStat, ResponseRecord, Stats, User } from './types'

const LS_MOCK = 'omuomu.mockResponses'

export class SessionExpired extends Error {}

export function isMock() {
  return store.localPreview || !store.pub?.sheetUrl
}

export function emptyStat(): PhotoStat {
  return { n: 0, people: {}, unknown: 0, left: 0, staff: 0 }
}

/** Apps Script 側と同じ集計（ローカル確認用） */
export function aggregate(rows: ResponseRecord[]): Stats {
  const s: Stats = {}
  for (const r of rows) addTo((s[r.photoId] ??= emptyStat()), r)
  return s
}

export function addTo(st: PhotoStat, r: Pick<ResponseRecord, 'people' | 'unknown' | 'left' | 'staff'>) {
  st.n += 1
  for (const p of new Set(r.people)) st.people[p] = (st.people[p] ?? 0) + 1
  if (r.unknown > 0) st.unknown += 1
  if (r.left > 0) st.left += 1
  if (r.staff > 0) st.staff += 1
}

function mockRows(): (ResponseRecord & { email?: string; t?: number })[] {
  try {
    return JSON.parse(localStorage.getItem(LS_MOCK) ?? '[]')
  } catch {
    return []
  }
}

/** text/plain で送ると CORS のプリフライトが発生しない */
async function post<T>(body: Record<string, unknown>): Promise<T> {
  const url = store.pub?.sheetUrl
  if (!url) throw new Error('Apps Script の URL が設定されていません')
  const res = await fetch(url, {
    method: 'POST',
    headers: { 'Content-Type': 'text/plain;charset=utf-8' },
    body: JSON.stringify(body),
  })
  const j = await res.json().catch(() => ({ error: 'failed', message: `応答を読めません (${res.status})` }))
  if (j.error === 'session') {
    logout()
    throw new SessionExpired(j.message ?? 'もう一度ログインしてください')
  }
  if (j.error) throw new Error(j.message ?? j.error)
  return j as T
}

function authed<T>(action: string, body: Record<string, unknown> = {}) {
  return post<T>({ action, session: store.user?.session, ...body })
}

export async function login(idToken: string): Promise<{ user: User; viewKey: string }> {
  const r = await post<{ session: string; email: string; name: string; role: User['role']; exp: number; viewKey: string }>({
    action: 'login',
    idToken,
    ua: navigator.userAgent,
  })
  return { user: { email: r.email, name: r.name, role: r.role, exp: r.exp, session: r.session }, viewKey: r.viewKey }
}

export async function fetchStats(): Promise<Stats> {
  if (!store.manifest) return {}
  try {
    if (isMock()) store.stats = aggregate(mockRows())
    else store.stats = (await authed<{ photos: Stats }>('stats')).photos ?? {}
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
  if (isMock()) {
    const rows = mockRows()
    rows.push({ ...r, email: store.user?.email, t: Date.now() })
    try {
      localStorage.setItem(LS_MOCK, JSON.stringify(rows))
    } catch {
      /* noop */
    }
  } else {
    await authed('submit', { ...r })
  }
  // 画面上の集計にもすぐ反映する
  addTo((store.stats[r.photoId] ??= emptyStat()), r)
}

/** 操作ログ（失敗しても画面の操作は止めない） */
export function logEvent(event: string, detail: Record<string, unknown> = {}) {
  if (isMock()) {
    console.info('[log]', store.user?.email, event, detail)
    return
  }
  authed('log', { event, detail }).catch((e) => console.warn('log failed', e))
}

export async function fetchAudit(limit = 300): Promise<AuditRow[]> {
  if (isMock()) {
    return mockRows()
      .map((r) => ({
        t: r.t ?? 0,
        email: r.email ?? '',
        name: '',
        role: '',
        action: '回答（この PC）',
        detail: JSON.stringify({ photo: r.photoTitle, names: r.names.join('、'), unknown: r.unknown, left: r.left, staff: r.staff }),
      }))
      .reverse()
  }
  return (await authed<{ rows: AuditRow[] }>('audit', { limit })).rows
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
