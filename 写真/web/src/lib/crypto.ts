// Rust(WASM) で鍵の導出・復号・ZIP 作成を行う薄いラッパー
import init, * as wasm from '../wasm/pkg/gallery_wasm.js'

let ready: Promise<unknown> | null = null
export function loadWasm() {
  ready ??= init()
  return ready
}

export async function decrypt(key: Uint8Array, data: Uint8Array): Promise<Uint8Array> {
  await loadWasm()
  return wasm.decrypt(key, data)
}

export async function fetchDecrypt(key: Uint8Array, url: string): Promise<Uint8Array> {
  const res = await fetch(url, { cache: 'no-cache' })
  if (!res.ok) throw new Error(`${url} を取得できません (${res.status})`)
  return decrypt(key, new Uint8Array(await res.arrayBuffer()))
}

export async function weightedPick(weights: number[]): Promise<number> {
  await loadWasm()
  const r = crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32
  return wasm.weighted_pick(new Float64Array(weights), r)
}

export async function makeZip(names: string[], files: Uint8Array[]): Promise<Uint8Array> {
  await loadWasm()
  return wasm.make_zip(names, files as unknown as any[])
}

export function b64ToBytes(b64: string): Uint8Array {
  return Uint8Array.from(atob(b64), (c) => c.charCodeAt(0))
}
export function bytesToB64(b: Uint8Array): string {
  let s = ''
  b.forEach((x) => (s += String.fromCharCode(x)))
  return btoa(s)
}
