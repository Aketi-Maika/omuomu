// Google でログイン（Google Identity Services）
// ブラウザは Google から ID トークン（ログイン証明）を受け取り、NAS がそれを Google に確かめる
import { login, signIn } from './api'
import { clearImages } from './images'
import { loadPublic, logout, store } from './store'

declare global {
  interface Window {
    google?: any
  }
}

let gis: Promise<void> | null = null
function loadGis(): Promise<void> {
  gis ??= new Promise((resolve, reject) => {
    const s = document.createElement('script')
    s.src = 'https://accounts.google.com/gsi/client'
    s.async = true
    s.onload = () => resolve()
    s.onerror = () => {
      gis = null
      reject(new Error('Google のログイン部品を読み込めませんでした'))
    }
    document.head.appendChild(s)
  })
  return gis
}

/** Google ログインが使える場所か（Google は http では localhost しか許可しない） */
export function googleUsableHere() {
  return location.protocol === 'https:' || ['localhost', '127.0.0.1'].includes(location.hostname)
}

/** el に「Google でログイン」ボタンを描画する */
export async function renderGoogleButton(
  el: HTMLElement,
  handlers: { onStart: () => void; onDone: () => void; onError: (msg: string) => void },
) {
  const pub = await loadPublic()
  if (!googleUsableHere()) {
    handlers.onError('このアドレスでは Google ログインを使えません。SSH トンネル経由で localhost から開いてください')
    return
  }
  if (!pub?.googleClientId || !store.apiBase) {
    handlers.onError('Google ログインがまだ設定されていません（管理者にお問い合わせください）')
    return
  }
  await loadGis()
  const g = window.google.accounts.id
  g.initialize({
    client_id: pub.googleClientId,
    // ドメインが 1 つならアカウント選択でそのドメインを優先（判定は NAS 側で hd を確認する）
    hd: pub.allowedDomains.length === 1 ? pub.allowedDomains[0] : undefined,
    ux_mode: 'popup',
    auto_select: false,
    callback: async (resp: { credential: string }) => {
      handlers.onStart()
      try {
        await signIn(await login(resp.credential))
        handlers.onDone()
      } catch (e) {
        logout()
        handlers.onError((e as Error).message || 'ログインできませんでした')
      }
    },
  })
  el.innerHTML = ''
  g.renderButton(el, { theme: 'outline', size: 'large', shape: 'rectangular', text: 'signin_with', locale: 'ja', width: 280 })
}

/** 管理ツールの中で開いているときだけ: Google ログインなしで表示を確認 */
export async function signInLocalPreview(): Promise<boolean> {
  await loadPublic()
  if (!store.isLocalTool) return false
  const r = await fetch('./api/dev-session')
  if (!r.ok) return false
  const { session } = await r.json()
  await signIn({ email: 'local@管理ツール', name: '管理ツール（確認用）', role: 'admin', exp: Date.now() + 12 * 3600 * 1000, session }, true)
  return true
}

export function signOut() {
  logout()
  clearImages()
  try {
    window.google?.accounts.id.disableAutoSelect()
  } catch {
    /* noop */
  }
}
