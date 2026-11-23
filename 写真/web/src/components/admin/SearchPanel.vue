<script setup lang="ts">
// 人物を選ぶ → その人が写っている写真を一覧 → ZIP で一括ダウンロード
import { computed, inject, ref, watch } from 'vue'
import EncImg from '../EncImg.vue'
import { downloadZip as fetchZip, emptyStat, logEvent } from '../../lib/api'
import { local as localApi, type LocalStatus } from '../../lib/local'
import { store } from '../../lib/store'
import { FIXED, type Photo } from '../../lib/types'

const props = defineProps<{ local: LocalStatus | null }>()
const toast = inject<(m: string) => void>('toast')!
const m = store.manifest!

const q = ref('')
const picked = ref<string[]>([])
const mode = ref<'and' | 'or'>('and')
const majority = ref(Math.round((m.majority ?? 0.5) * 100))
const useSeed = ref(true)
const unchecked = ref(new Set<string>())
const zipping = ref(0)
const exportDest = ref('')
const exportMsg = ref('')

const fixedIds = FIXED.map((f) => ({ id: f.id, name: f.label, group: '名簿にいない人', key: f.key }))
const people = computed(() => {
  const nq = q.value.normalize('NFKC').replace(/\s+/g, '')
  return [...m.roster, ...fixedIds].filter((p) => !nq || p.name.replace(/\s+/g, '').includes(nq))
})
const nameOf = (id: string) => m.roster.find((p) => p.id === id)?.name ?? FIXED.find((f) => f.id === id)?.label ?? id

/** その写真にその人が写っているとみなせるか。votes は得票数 */
function judge(p: Photo, id: string): { hit: boolean; votes: number; n: number; seed: boolean } {
  const st = store.stats[p.id] ?? emptyStat()
  const f = FIXED.find((x) => x.id === id)
  const votes = f ? st[f.key] : st.people[id] ?? 0
  const seed = useSeed.value && (p.seed ?? []).includes(id)
  const hit = seed || (st.n > 0 && votes / st.n >= majority.value / 100)
  return { hit, votes, n: st.n, seed }
}

const results = computed(() => {
  if (!picked.value.length) return []
  return m.photos
    .map((p, i) => {
      const js = picked.value.map((id) => judge(p, id))
      const ok = mode.value === 'and' ? js.every((j) => j.hit) : js.some((j) => j.hit)
      return { p, no: i + 1, ok, js }
    })
    .filter((r) => r.ok)
})
const checked = computed(() => results.value.filter((r) => !unchecked.value.has(r.p.id)))
watch(picked, () => (unchecked.value = new Set()))

// 誰の写真を探したかをログに残す（選び終わるまで少し待ってから 1 回）
let logTimer: number | undefined
watch([picked, mode], () => {
  clearTimeout(logTimer)
  if (!picked.value.length) return
  logTimer = window.setTimeout(
    () => logEvent('人物で絞り込み', { people: picked.value.map(nameOf), mode: mode.value, count: results.value.length }),
    1500,
  )
})

function toggle(id: string) {
  picked.value = picked.value.includes(id) ? picked.value.filter((x) => x !== id) : [...picked.value, id]
}
function toggleCheck(id: string) {
  const s = new Set(unchecked.value)
  s.has(id) ? s.delete(id) : s.add(id)
  unchecked.value = s
}
function voteLabel(r: (typeof results.value)[number]) {
  const j = r.js[0]
  if (r.js.length === 1) return j.seed && j.n === 0 ? '登録済み' : `${j.votes}/${j.n}票${j.seed ? '・登録' : ''}`
  const n = j.n
  return n ? `回答 ${n}件` : '登録済み'
}

const who = computed(() => {
  const names = picked.value.map(nameOf).join(' ・ ')
  if (!names) return ''
  return picked.value.length === 1 ? `${names} が写っている写真` : mode.value === 'and' ? `${names} が一緒に写っている写真` : `${names} の誰かが写っている写真`
})

async function downloadZip() {
  const list = checked.value
  if (!list.length) return
  zipping.value = 1
  try {
    const label = picked.value.map(nameOf).join('_').slice(0, 40)
    const blob = await fetchZip(
      list.map((r) => r.p.id),
      label,
      picked.value.map(nameOf),
    )
    const a = document.createElement('a')
    a.href = URL.createObjectURL(blob)
    a.download = `写真_${label || '選択'}.zip`
    a.click()
    setTimeout(() => URL.revokeObjectURL(a.href), 10000)
    toast(`${list.length} 点を ZIP にしました（NAS に記録されました）`)
  } catch (e) {
    console.error(e)
    toast('ZIP を作れませんでした: ' + (e as Error).message)
  } finally {
    zipping.value = 0
  }
}

async function exportOriginals() {
  exportMsg.value = '書き出しています…'
  try {
    const r = await localApi.exportOriginals(
      checked.value.map((x) => x.p.id),
      exportDest.value,
    )
    const ok = Object.values(r.results).filter((v) => !v.startsWith('コピーできません') && !v.startsWith('元ファイル')).length
    exportMsg.value = `${ok} 点を ${r.dest} に書き出しました`
    logEvent('原寸の書き出し', { people: picked.value.map(nameOf), count: ok, dest: r.dest, photos: checked.value.map((x) => x.p.title) })
  } catch (e) {
    exportMsg.value = String((e as Error).message)
  }
}
</script>

<template>
  <div class="layout">
    <aside class="side">
      <h2 class="serif">人物を選ぶ</h2>
      <label class="search"><span aria-hidden="true">⌕</span><input v-model="q" type="search" placeholder="名前で検索" /></label>
      <div class="people">
        <button
          v-for="p in people"
          :key="p.id"
          class="chip"
          :class="{ on: picked.includes(p.id), fx: p.id.startsWith('__') }"
          @click="toggle(p.id)"
        >
          {{ p.name }}
        </button>
      </div>

      <div class="mode" role="radiogroup" aria-label="絞り込み方">
        <button :class="{ on: mode === 'or' }" role="radio" :aria-checked="mode === 'or'" @click="mode = 'or'">いずれか (OR)</button>
        <button :class="{ on: mode === 'and' }" role="radio" :aria-checked="mode === 'and'" @click="mode = 'and'">全員 (AND)</button>
      </div>
      <p class="note">AND：選んだ全員が一緒に写っている写真<br />OR：誰か一人でも写っている写真</p>

      <div class="label mt">判定のしきい値</div>
      <p class="th">回答者の <b class="latin">{{ majority }}%</b> 以上が選んだ人を「写っている」とみなす</p>
      <input v-model.number="majority" type="range" min="10" max="100" step="5" class="range" aria-label="しきい値" />
      <label class="check"><input v-model="useSeed" type="checkbox" /> 名簿（Excel）の登録済みデータも使う</label>
      <p v-if="store.statsError" class="warn">{{ store.statsError }}</p>
    </aside>

    <section class="main">
      <div class="head">
        <div>
          <div class="count latin">{{ results.length }}<small>点の作品</small></div>
          <div class="who serif">{{ who || '左の名簿から人物を選んでください' }}</div>
        </div>
        <div class="dl">
          <button class="btn fill" :disabled="!checked.length || zipping > 0" @click="downloadZip">
            {{ zipping ? 'ZIP を作成中…' : `選択した ${checked.length} 点を ZIP でダウンロード` }}
          </button>
        </div>
      </div>

      <div class="grid">
        <button v-for="r in results" :key="r.p.id" class="cell" :class="{ off: unchecked.has(r.p.id) }" @click="toggleCheck(r.p.id)">
          <span class="check-mark" aria-hidden="true">✓</span>
          <div class="frame"><div class="mat"><EncImg :id="r.p.id" thumb :alt="r.p.title" /></div></div>
          <div class="cap"><b class="latin">No.{{ String(r.no).padStart(3, '0') }}</b><span>{{ voteLabel(r) }}</span></div>
        </button>
      </div>

      <div v-if="props.local && results.length" class="export">
        <div class="label">原寸データを書き出す（この PC のみ）</div>
        <div class="row">
          <input v-model="exportDest" class="input" placeholder="例：D:\書き出し\山田さん" />
          <button class="btn small" :disabled="!exportDest || !checked.length" @click="exportOriginals">NAS の元ファイルをコピー</button>
        </div>
        <p class="note">{{ exportMsg }}</p>
      </div>
    </section>
  </div>
</template>

<style scoped>
.layout {
  display: grid;
  grid-template-columns: 360px 1fr;
  min-height: calc(100dvh - var(--top-h));
}
.side {
  padding: 32px 32px 40px;
  border-right: 1px solid var(--line);
  background: var(--paper-2);
}
.side h2 {
  font-weight: 500;
  font-size: 21px;
  letter-spacing: 0.12em;
  margin-bottom: 18px;
}
.search {
  display: flex;
  gap: 8px;
  align-items: center;
  border-bottom: 1px solid var(--ink);
  padding: 4px 2px;
  color: var(--ink-soft);
  margin-bottom: 16px;
}
.search input {
  flex: 1;
  border: none;
  background: transparent;
  padding: 6px 0;
  font-size: 14px;
  min-width: 0;
}
.search input:focus {
  outline: none;
}
.people {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
  max-height: 40vh;
  overflow-y: auto;
}
.mode {
  display: flex;
  margin-top: 24px;
  border: 1px solid var(--ink);
  width: fit-content;
}
.mode button {
  padding: 8px 16px;
  font-size: 12px;
  letter-spacing: 0.12em;
}
.mode .on {
  background: var(--ink);
  color: var(--wall);
}
.note {
  font-size: 11px;
  color: var(--ink-soft);
  margin-top: 10px;
  line-height: 1.8;
}
.mt {
  margin-top: 26px;
}
.th {
  font-size: 13px;
  margin-top: 8px;
}
.th b {
  font-size: 20px;
  font-weight: 400;
}
.range {
  width: 100%;
  margin-top: 8px;
  accent-color: var(--accent);
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 12px;
  margin-top: 14px;
}
.check input {
  accent-color: var(--accent);
}
.warn {
  margin-top: 14px;
  font-size: 12px;
  color: var(--dot);
}
.main {
  padding: 32px 40px 48px;
  min-width: 0;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 20px;
  flex-wrap: wrap;
  border-bottom: 1px solid var(--line);
  padding-bottom: 20px;
}
.count {
  font-size: 44px;
  line-height: 1;
}
.count small {
  font-family: var(--sans);
  font-size: 13px;
  color: var(--ink-soft);
  margin-left: 8px;
}
.who {
  font-size: 15px;
  margin-top: 10px;
  letter-spacing: 0.06em;
}
.grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  gap: 22px;
  margin-top: 26px;
}
.cell {
  position: relative;
  text-align: left;
  transition: opacity 0.2s;
}
.cell.off {
  opacity: 0.35;
}
.cell .frame {
  padding: 6px;
  box-shadow: 0 10px 18px -10px var(--shadow);
}
.cell .mat {
  padding: 8px;
  aspect-ratio: 4 / 3;
}
.cell :deep(img) {
  object-fit: cover;
}
.check-mark {
  position: absolute;
  top: 12px;
  right: 12px;
  z-index: 2;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  background: var(--accent);
  color: var(--accent-ink);
  font-size: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
}
.cell.off .check-mark {
  background: var(--paper);
  color: transparent;
  border: 1px solid var(--line);
}
.cap {
  display: flex;
  justify-content: space-between;
  font-size: 11px;
  color: var(--ink-soft);
  margin-top: 8px;
}
.cap b {
  font-size: 13px;
  color: var(--ink);
}
.export {
  margin-top: 40px;
  padding-top: 20px;
  border-top: 1px solid var(--line);
  max-width: 640px;
}
.export .row {
  display: flex;
  gap: 12px;
  align-items: flex-end;
  margin-top: 8px;
}

@media (max-width: 760px) {
  .layout {
    display: block;
  }
  .side {
    border-right: none;
    border-bottom: 1px solid var(--line);
    padding: 16px 18px;
  }
  .side h2 {
    display: none;
  }
  .people {
    flex-wrap: nowrap;
    overflow-x: auto;
    max-height: none;
    padding-bottom: 4px;
  }
  .people .chip {
    flex: none;
  }
  .main {
    padding: 20px 18px 110px;
  }
  .count {
    font-size: 34px;
  }
  .grid {
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }
  .cell .frame {
    padding: 3px;
  }
  .cell .mat {
    padding: 4px;
  }
  .cap {
    font-size: 10px;
    flex-direction: column;
  }
  .check-mark {
    top: 6px;
    right: 6px;
    width: 18px;
    height: 18px;
  }
  .dl {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 0;
    z-index: 10;
    padding: 12px 16px calc(14px + env(safe-area-inset-bottom));
    background: var(--paper);
    border-top: 1px solid var(--line);
  }
  .dl .btn {
    width: 100%;
    letter-spacing: 0.1em;
  }
}
</style>
