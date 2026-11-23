// 画面間で共有する状態（ログイン・名簿・写真一覧・集計）
import { reactive } from 'vue'
import { localStatus } from './local'
import type { Manifest, PublicInfo, Stats, User } from './types'

const SS_SESSION = 'omuomu.session.v3'

export const DATA = './data/'

function ssGet(k: string) {
  try {
    return sessionStorage.getItem(k)
  } catch {
    return null
  }
}
function ssSet(k: string, v: string | null) {
  try {
    v === null ? sessionStorage.removeItem(k) : sessionStorage.setItem(k, v)
  } catch {
    /* プライベートモード等 */
  }
}

export const store = reactive({
  pub: null as PublicInfo | null,
  pubError: '' as string,
  /** 公開 API（NAS）の場所。管理ツールの中で開いているときは同じ場所 */
  apiBase: '' as string,
  isLocalTool: false,
  manifest: null as Manifest | null,
  user: null as User | null,
  /** 管理ツールから Google ログインなしで表示を確かめている */
  localPreview: false,
  stats: {} as Stats,
  statsLoaded: false,
  statsError: '',
})

export function isAdmin() {
  return store.user?.role === 'admin'
}

let pubLoading: Promise<PublicInfo | null> | null = null
export function loadPublic(force = false): Promise<PublicInfo | null> {
  if (store.pub && !force) return Promise.resolve(store.pub)
  if (pubLoading && !force) return pubLoading
  pubLoading = (async () => {
    const local = await localStatus()
    store.isLocalTool = !!local
    try {
      const r = await fetch(DATA + 'public.json', { cache: 'no-cache' })
      if (!r.ok) throw new Error(String(r.status))
      store.pub = await r.json()
      store.pubError = ''
      document.title = store.pub!.title || '写真展'
    } catch {
      store.pub = null
      store.pubError = 'まだ写真が公開されていません'
    }
    store.apiBase = local ? '.' : (store.pub?.apiBase ?? '').replace(/\/$/, '')
    return store.pub
  })()
  return pubLoading
}

export function saveSession() {
  if (store.user) ssSet(SS_SESSION, JSON.stringify({ user: store.user, localPreview: store.localPreview }))
}

/** タブ内で再読み込みしたときはログインし直さない（期限まで） */
export async function restoreSession(): Promise<void> {
  if (store.manifest) return
  await loadPublic()
  const raw = ssGet(SS_SESSION)
  if (!raw) return
  try {
    const s = JSON.parse(raw) as { user: User; localPreview: boolean }
    if (s.user.exp < Date.now()) throw new Error('expired')
    store.user = s.user
    store.localPreview = s.localPreview
    const { fetchManifest } = await import('./api')
    await fetchManifest()
  } catch {
    logout()
  }
}

export function logout() {
  ssSet(SS_SESSION, null)
  store.manifest = null
  store.user = null
  store.localPreview = false
  store.stats = {}
  store.statsLoaded = false
}
