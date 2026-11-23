<script setup lang="ts">
// 入力状況: 写真ごとの回答数（少ない順）
import { computed } from 'vue'
import EncImg from '../EncImg.vue'
import { fetchStats, isMock } from '../../lib/api'
import { store } from '../../lib/store'

const m = store.manifest!
const rows = computed(() =>
  m.photos
    .map((p, i) => ({ p, no: i + 1, st: store.stats[p.id] }))
    .sort((a, b) => (a.st?.n ?? 0) - (b.st?.n ?? 0) || a.no - b.no),
)
const totalAnswers = computed(() => Object.values(store.stats).reduce((a, s) => a + s.n, 0))
const full = computed(() => rows.value.filter((r) => (r.st?.n ?? 0) >= m.threshold).length)
const zero = computed(() => rows.value.filter((r) => !r.st?.n).length)
const nameOf = (id: string) => m.roster.find((p) => p.id === id)?.name ?? id
function top(st: (typeof rows.value)[number]['st']) {
  if (!st) return ''
  return Object.entries(st.people)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 4)
    .map(([id, c]) => `${nameOf(id)} ${c}`)
    .join('、')
}
</script>

<template>
  <section class="status">
    <div class="summary">
      <div><div class="label">回答の合計</div><div class="big latin">{{ totalAnswers }}</div></div>
      <div><div class="label">{{ m.threshold }}件以上の作品</div><div class="big latin">{{ full }}<small> / {{ m.photos.length }}</small></div></div>
      <div><div class="label">未回答の作品</div><div class="big latin">{{ zero }}</div></div>
      <button class="btn small" @click="fetchStats">最新の状態に更新</button>
    </div>
    <p v-if="isMock()" class="warn">スプレッドシートが未設定のため、この端末で記録した回答だけを表示しています。</p>
    <p v-if="store.statsError" class="warn">{{ store.statsError }}</p>

    <table>
      <thead>
        <tr><th>作品</th><th></th><th>回答</th><th>多く選ばれた人</th><th>不明 / 退会 / 教職員</th></tr>
      </thead>
      <tbody>
        <tr v-for="r in rows" :key="r.p.id">
          <td class="thumb"><EncImg :id="r.p.id" thumb /></td>
          <td><b class="latin">No.{{ String(r.no).padStart(3, '0') }}</b><br /><span class="soft">{{ r.p.title }}</span></td>
          <td class="n">
            <span class="meter"><i :style="{ width: Math.min(100, ((r.st?.n ?? 0) / m.threshold) * 100) + '%' }"></i></span>
            {{ r.st?.n ?? 0 }}
          </td>
          <td class="soft">{{ top(r.st) }}</td>
          <td class="soft">{{ r.st ? `${r.st.unknown} / ${r.st.left} / ${r.st.staff}` : '' }}</td>
        </tr>
      </tbody>
    </table>
  </section>
</template>

<style scoped>
.status {
  padding: 32px 40px 60px;
  max-width: 1200px;
  margin: 0 auto;
}
.summary {
  display: flex;
  gap: 48px;
  align-items: flex-end;
  flex-wrap: wrap;
  border-bottom: 1px solid var(--line);
  padding-bottom: 20px;
}
.big {
  font-size: 40px;
  line-height: 1.1;
}
.big small {
  font-size: 16px;
  color: var(--ink-soft);
}
.warn {
  margin-top: 14px;
  font-size: 12px;
  color: var(--dot);
}
table {
  width: 100%;
  border-collapse: collapse;
  margin-top: 20px;
  font-size: 13px;
}
th {
  text-align: left;
  font-weight: 400;
  font-size: 11px;
  letter-spacing: 0.2em;
  color: var(--ink-soft);
  padding: 8px;
  border-bottom: 1px solid var(--line);
}
td {
  padding: 8px;
  border-bottom: 1px solid var(--line);
  vertical-align: middle;
}
.thumb {
  width: 76px;
}
.thumb > * {
  width: 64px;
  height: 46px;
  border: 3px solid var(--frame);
}
.thumb :deep(img) {
  object-fit: cover;
}
b {
  font-weight: 400;
  font-size: 15px;
}
.soft {
  color: var(--ink-soft);
  font-size: 12px;
}
.n {
  white-space: nowrap;
}
.meter {
  display: inline-block;
  width: 70px;
  height: 4px;
  background: var(--line);
  vertical-align: middle;
  margin-right: 8px;
}
.meter i {
  display: block;
  height: 100%;
  background: var(--accent);
}
@media (max-width: 760px) {
  .status {
    padding: 20px 14px 40px;
  }
  .summary {
    gap: 24px;
  }
  .big {
    font-size: 30px;
  }
  th:nth-child(4),
  td:nth-child(4),
  th:nth-child(5),
  td:nth-child(5) {
    display: none;
  }
}
</style>
