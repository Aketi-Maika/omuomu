// PC のローカル管理ツール（omuomu-gallery）の API。GitHub Pages 上では存在しない
export interface LocalStatus {
  local: true
  root: string
  git: { branch: string | null; remote: string | null }
}

export interface LocalConfig {
  photoDirs: string[]
  rosterPath: string
  googleClientId: string
  allowedDomain: string
  adminEmails: string[]
  sheetUrl: string
  viewKey: string
  title: string
  subtitle: string
  period: string
  notice: string
  threshold: number
  lowWeight: number
  majority: number
  maxSize: number
  thumbSize: number
  jpegQuality: number
  outDir: string
}

export interface BuildStatus {
  running: boolean
  done: number
  total: number
  phase: string
  log: string[]
  error: string | null
  report: {
    photos: number
    encoded: number
    reused: number
    removed: number
    people: number
    seedLinks: number
    unmatchedRefs: number
    seconds: number
    warnings: string[]
  } | null
}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const r = await fetch('./api' + path, {
    ...init,
    headers: init?.body ? { 'Content-Type': 'application/json' } : undefined,
  })
  const j = await r.json().catch(() => ({ error: `応答を読めません (${r.status})` }))
  if (!r.ok || j.error) throw new Error(j.error ?? String(r.status))
  return j as T
}

export async function localStatus(): Promise<LocalStatus | null> {
  try {
    const r = await fetch('./api/status', { cache: 'no-store' })
    if (!r.ok) return null
    const j = await r.json()
    return j.local ? j : null
  } catch {
    return null
  }
}

export const local = {
  getConfig: () => call<LocalConfig>('/config'),
  saveConfig: (c: LocalConfig) =>
    call<{ ok: boolean; problems: string[]; warnings: string[] }>('/config', { method: 'PUT', body: JSON.stringify(c) }),
  newKey: () => call<{ ok: boolean }>('/config/new-key', { method: 'POST' }),
  appsScriptSetup: () => call<{ code: string }>('/apps-script-setup'),
  checkPath: (p: string) =>
    call<{ exists: boolean; kind?: string; images?: number; path: string }>('/check-path?p=' + encodeURIComponent(p)),
  build: () => call<{ ok: boolean }>('/build', { method: 'POST' }),
  buildStatus: () => call<BuildStatus>('/build/status'),
  publish: (message: string) =>
    call<{ committed: boolean; pushed: boolean; branch: string; log: string[] }>('/publish', {
      method: 'POST',
      body: JSON.stringify({ message }),
    }),
  exportOriginals: (ids: string[], dest: string) =>
    call<{ dest: string; results: Record<string, string> }>('/export', { method: 'POST', body: JSON.stringify({ ids, dest }) }),
}
