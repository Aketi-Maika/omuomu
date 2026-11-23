/** public.json（平文。写真や名前は含まない） */
export interface PublicInfo {
  v: number
  title: string
  subtitle: string
  period: string
  notice: string
  count: number
  googleClientId: string
  allowedDomains: string[]
  sheetUrl: string
  keyCheck: string
  builtAt: number
}

/** ログイン中の利用者（メールアドレスは Apps Script が Google のログイン証明から確認したもの） */
export interface User {
  email: string
  name: string
  role: 'viewer' | 'admin'
  exp: number
  session: string
}

export interface Person {
  id: string
  name: string
  group: string
}

export interface Photo {
  id: string
  w: number
  h: number
  title: string
  folder: string
  /** 名簿(Excel)に登録済みの「写っている人」 */
  seed: string[]
}

/** manifest.enc の中身 */
export interface Manifest {
  title: string
  groups: string[]
  roster: Person[]
  photos: Photo[]
  threshold: number
  lowWeight: number
  majority: number
}

/** 名簿にいない人（いつでも選べる固定の選択肢） */
export const FIXED = [
  { key: 'unknown', id: '__unknown', label: '不明' },
  { key: 'left', id: '__left', label: '退会' },
  { key: 'staff', id: '__staff', label: '教職員' },
] as const
export type FixedKey = (typeof FIXED)[number]['key']
export type FixedCounts = Record<FixedKey, number>

/** 1 回の「記録」 */
export interface ResponseRecord {
  photoId: string
  photoTitle: string
  people: string[]
  names: string[]
  unknown: number
  left: number
  staff: number
  sessionId: string
}

/** 管理室の「ログ」 */
export interface AuditRow {
  t: number
  email: string
  name: string
  role: string
  action: string
  detail: string
}

/** 写真ごとの集計 */
export interface PhotoStat {
  /** 回答件数 */
  n: number
  /** 人物 ID → その人を選んだ回答の数 */
  people: Record<string, number>
  /** 不明/退会/教職員を 1 人以上選んだ回答の数 */
  unknown: number
  left: number
  staff: number
}
export type Stats = Record<string, PhotoStat>
