// 画面間で共有する状態（ログイン・名簿・写真一覧・集計）
import { reactive } from 'vue'
import { b64ToBytes, fetchDecrypt } from './crypto'
import type { Manifest, PublicInfo, Stats, User } from './types'

const SS_SESSION = 'omuomu.session.v2'

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
  key: null as Uint8Array | null,
  manifest: null as Manifest | null,
  user: null as User | null,
  /** この PC の管理ツールから、Google ログインなしで表示確認している */
  localPreview: false,
  stats: {} as Stats,
  statsLoaded: false,
  statsError: '',
})

export function isAdmin() {
  return store.user?.role === 'admin'
}

export async function loadPublic(force = false): Promise<PublicInfo | null> {
  if (store.pub && !force) return store.pub
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
  return store.pub
}

/** 写真の鍵で名簿・写真一覧を開く */
export async function openWithKey(viewKeyB64: string): Promise<void> {
  const key = b64ToBytes(viewKeyB64)
  const bytes = await fetchDecrypt(key, DATA + 'manifest.enc')
  store.manifest = JSON.parse(new TextDecoder().decode(bytes))
  store.key = key
}

export async function signIn(user: User, viewKeyB64: string, localPreview = false) {
  await openWithKey(viewKeyB64)
  store.user = user
  store.localPreview = localPreview
  ssSet(SS_SESSION, JSON.stringify({ user, viewKey: viewKeyB64, localPreview }))
}

/** タブ内で再読み込みしたときはログインし直さない（セッションの期限まで） */
export async function restoreSession(): Promise<void> {
  if (store.manifest) return
  await loadPublic()
  if (!store.pub) return
  const raw = ssGet(SS_SESSION)
  if (!raw) return
  try {
    const s = JSON.parse(raw) as { user: User; viewKey: string; localPreview: boolean }
    if (!s.localPreview && s.user.exp < Date.now()) throw new Error('expired')
    await openWithKey(s.viewKey)
    store.user = s.user
    store.localPreview = s.localPreview
  } catch {
    // 期限切れ、またはデータが作り直されて鍵が変わった
    logout()
  }
}

export function logout() {
  ssSet(SS_SESSION, null)
  store.key = null
  store.manifest = null
  store.user = null
  store.localPreview = false
  store.stats = {}
  store.statsLoaded = false
}
