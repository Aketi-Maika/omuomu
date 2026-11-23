// Google でログイン（Google Identity Services）
// ブラウザは Google から ID トークン（ログイン証明）を受け取り、Apps Script がそれを検証する
import { login } from './api'
import { localStatus } from './local'
import { signIn, store } from './store'

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
  const pub = store.pub
  if (!googleUsableHere()) {
    handlers.onError('LAN のアドレスからは Google ログインを使えません。下の「管理ツールで確認」から入ってください')
    return
  }
  if (!pub?.googleClientId || !pub.sheetUrl) {
    handlers.onError('Google ログインがまだ設定されていません（管理者にお問い合わせください）')
    return
  }
  await loadGis()
  const g = window.google.accounts.id
  g.initialize({
    client_id: pub.googleClientId,
    // ドメインが 1 つならアカウント選択でそのドメインを優先
    hd: pub.allowedDomains.length === 1 ? pub.allowedDomains[0] : undefined,
    ux_mode: 'popup',
    auto_select: false,
    callback: async (resp: { credential: string }) => {
      handlers.onStart()
      try {
        const { user, viewKey } = await login(resp.credential)
        await signIn(user, viewKey)
        handlers.onDone()
      } catch (e) {
        handlers.onError((e as Error).message || 'ログインできませんでした')
      }
    },
  })
  el.innerHTML = ''
  g.renderButton(el, { theme: 'outline', size: 'large', shape: 'rectangular', text: 'signin_with', locale: 'ja', width: 280 })
}

/** この PC の管理ツールで開いているときだけ: Google ログインなしで表示を確認 */
export async function signInLocalPreview(): Promise<boolean> {
  const st = await localStatus()
  if (!st) return false
  const r = await fetch('./api/dev-key')
  if (!r.ok) return false
  const { viewKey } = await r.json()
  await signIn({ email: '管理ツール', name: '管理ツール（確認用）', role: 'admin', exp: Number.MAX_SAFE_INTEGER, session: '' }, viewKey, true)
  return true
}

export function googleSignOut() {
  try {
    window.google?.accounts.id.disableAutoSelect()
  } catch {
    /* noop */
  }
}
