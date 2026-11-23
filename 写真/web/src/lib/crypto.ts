// Rust(WASM): 重み付き抽選（回答が集まった写真ほど出にくくする）
import init, * as wasm from '../wasm/pkg/gallery_wasm.js'

let ready: Promise<unknown> | null = null
export function loadWasm() {
  ready ??= init()
  return ready
}

export async function weightedPick(weights: number[]): Promise<number> {
  await loadWasm()
  const r = crypto.getRandomValues(new Uint32Array(1))[0] / 2 ** 32
  return wasm.weighted_pick(new Float64Array(weights), r)
}
