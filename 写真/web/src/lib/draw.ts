// 次に出す写真の抽選。回答が threshold 件以上集まった写真は lowWeight 倍しか出ない
import { weightedPick } from './crypto'
import type { Manifest, Stats } from './types'

export function weightsFor(m: Manifest, stats: Stats, exclude: Set<string>): number[] {
  const allFull = m.photos.every((p) => (stats[p.id]?.n ?? 0) >= m.threshold)
  return m.photos.map((p) => {
    if (exclude.has(p.id)) return 0
    if (allFull) return 1
    return (stats[p.id]?.n ?? 0) >= m.threshold ? m.lowWeight : 1
  })
}

/**
 * @param seen このセッションで回答・スキップした写真（なるべく出さない）
 * @param current 今表示している写真（連続で出さない）
 */
export async function drawNext(m: Manifest, stats: Stats, seen: Set<string>, current?: string): Promise<string | null> {
  if (!m.photos.length) return null
  const ex = new Set(seen)
  if (current) ex.add(current)
  let w = weightsFor(m, stats, ex)
  if (w.every((x) => x <= 0)) {
    // 全部見終わったら、直前の 1 枚以外から改めて抽選
    w = weightsFor(m, stats, new Set(current && m.photos.length > 1 ? [current] : []))
  }
  const i = await weightedPick(w)
  return i >= 0 ? m.photos[i].id : null
}
