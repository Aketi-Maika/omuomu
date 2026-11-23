// 暗号化された写真を取得・復号して object URL にする（同じ写真は使い回す）
import { fetchDecrypt } from './crypto'
import { DATA, store } from './store'

const cache = new Map<string, Promise<string>>()
const order: string[] = []
const LIMIT = 80

export function imageUrl(id: string, thumb = false): Promise<string> {
  const k = id + (thumb ? '_t' : '')
  let p = cache.get(k)
  if (!p) {
    const key = store.key
    if (!key) return Promise.reject(new Error('locked'))
    p = fetchDecrypt(key, `${DATA}img/${k}.bin`).then((b) => URL.createObjectURL(new Blob([b], { type: 'image/jpeg' })))
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

export async function imageBytes(id: string): Promise<Uint8Array> {
  return fetchDecrypt(store.key!, `${DATA}img/${id}.bin`)
}

export function preload(id: string) {
  imageUrl(id).catch(() => {})
}
