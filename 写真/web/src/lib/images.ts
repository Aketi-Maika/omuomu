// NAS から写真を 1 枚ずつ取得して object URL にする（同じ写真は使い回す）
// 写真はログインを確かめてから NAS が返す。ブラウザのメモリにだけ置き、タブを閉じると消える
import { call } from './api'

const cache = new Map<string, Promise<string>>()
const order: string[] = []
const LIMIT = 80

export function imageUrl(id: string, thumb = false): Promise<string> {
  const k = id + (thumb ? '_t' : '')
  let p = cache.get(k)
  if (!p) {
    p = call(`/pub/photo/${id}${thumb ? '?t=1' : ''}`)
      .then((r) => r.blob())
      .then((b) => URL.createObjectURL(b))
    p.catch(() => cache.delete(k))
    cache.set(k, p)
    order.push(k)
    while (order.length > LIMIT) {
      const old = order.shift()!
      cache.get(old)?.then((u) => URL.revokeObjectURL(u))
      cache.delete(old)
    }
  }
  return p
}

export function clearImages() {
  for (const p of cache.values()) p.then((u) => URL.revokeObjectURL(u)).catch(() => {})
  cache.clear()
  order.length = 0
}

export function preload(id: string) {
  imageUrl(id).catch(() => {})
}
