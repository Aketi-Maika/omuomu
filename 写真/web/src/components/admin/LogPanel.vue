<script setup lang="ts">
// ログ: 誰が・いつ・何をしたか（入場、回答、管理室での絞り込み・ダウンロード）
import { computed, onMounted, ref } from 'vue'
import { fetchAudit, isMock } from '../../lib/api'
import type { AuditRow } from '../../lib/types'

const rows = ref<AuditRow[]>([])
const loading = ref(false)
const error = ref('')
const q = ref('')
const kind = ref('')

async function load() {
  loading.value = true
  error.value = ''
  try {
    rows.value = await fetchAudit(500)
  } catch (e) {
    error.value = 'ログを読み込めませんでした: ' + (e as Error).message
  } finally {
    loading.value = false
  }
}
onMounted(load)

const kinds = computed(() => [...new Set(rows.value.map((r) => r.action))])
const shown = computed(() => {
  const nq = q.value.trim().toLowerCase()
  return rows.value.filter(
    (r) => (!kind.value || r.action === kind.value) && (!nq || `${r.email} ${r.name} ${r.detail}`.toLowerCase().includes(nq)),
  )
})

function fmt(t: number) {
  if (!t) return ''
  const d = new Date(t)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${d.getFullYear()}/${p(d.getMonth() + 1)}/${p(d.getDate())} ${p(d.getHours())}:${p(d.getMinutes())}`
}

/** 内容(JSON)を読みやすく */
function detail(r: AuditRow) {
  try {
    const j = JSON.parse(r.detail)
    const parts: string[] = []
    if (j.photo) parts.push(j.photo)
    if (j.names) parts.push(j.names)
    for (const [k, l] of [
      ['unknown', '不明'],
      ['left', '退会'],
      ['staff', '教職員'],
    ] as const)
      if (Number(j[k]) > 0) parts.push(`${l}×${j[k]}`)
    if (j.people) parts.push(`人物: ${j.people.join('、')}`)
    if (j.mode) parts.push(j.mode === 'and' ? '全員(AND)' : 'いずれか(OR)')
    if (j.count != null) parts.push(`${j.count}点`)
    if (j.photos) parts.push(`写真: ${j.photos.join(', ')}`)
    if (j.tab) parts.push(j.tab)
    if (j.dest) parts.push(`書き出し先: ${j.dest}`)
    return parts.join(' ／ ') || (j.ua ? '' : r.detail)
  } catch {
    return r.detail
  }
}

function downloadCsv() {
  const esc = (s: string) => `"${String(s).replace(/"/g, '""')}"`
  const lines = [['日時', 'メール', '氏名', '操作', '内容'].join(',')]
  for (const r of shown.value) lines.push([fmt(r.t), r.email, r.name, r.action, detail(r)].map(esc).join(','))
  const a = document.createElement('a')
  a.href = URL.createObjectURL(new Blob(['﻿' + lines.join('\r\n')], { type: 'text/csv' }))
  a.download = 'omuomu_log.csv'
  a.click()
  setTimeout(() => URL.revokeObjectURL(a.href), 5000)
}
</script>

<template>
  <section class="log">
    <div class="head">
      <div>
        <h2 class="serif">ログ</h2>
        <p class="soft">
          誰が・いつ・何をしたかの記録です（新しい順・最大 500 件）。メールアドレスは Google のログイン証明から確認したものです。
          <template v-if="isMock()"><br />確認モードのため、この端末で記録した回答だけを表示しています。</template>
        </p>
      </div>
      <div class="tools">
        <label class="search"><span aria-hidden="true">⌕</span><input v-model="q" type="search" placeholder="メール・名前・内容で検索" /></label>
        <select v-model="kind" aria-label="操作の種類">
          <option value="">すべての操作</option>
          <option v-for="k in kinds" :key="k" :value="k">{{ k }}</option>
        </select>
        <button class="btn small" :disabled="loading" @click="load">{{ loading ? '読み込み中…' : '更新' }}</button>
        <button class="btn small" :disabled="!shown.length" @click="downloadCsv">CSV</button>
      </div>
    </div>
    <p v-if="error" class="err">{{ error }}</p>

    <table>
      <thead>
        <tr><th>日時</th><th>アカウント</th><th>操作</th><th>内容</th></tr>
      </thead>
      <tbody>
        <tr v-for="(r, i) in shown" :key="i">
          <td class="t">{{ fmt(r.t) }}</td>
          <td><div>{{ r.name }}</div><div class="soft">{{ r.email }}</div></td>
          <td><span class="act" :class="{ ans: r.action.startsWith('回答'), dl: r.action.includes('ダウンロード') || r.action.includes('書き出し') }">{{ r.action }}</span></td>
          <td class="d">{{ detail(r) }}</td>
        </tr>
        <tr v-if="!loading && !shown.length"><td colspan="4" class="soft empty">記録がありません</td></tr>
      </tbody>
    </table>
  </section>
</template>

<style scoped>
.log {
  padding: 32px 40px 60px;
  max-width: 1280px;
  margin: 0 auto;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 20px;
  flex-wrap: wrap;
  border-bottom: 1px solid var(--line);
  padding-bottom: 18px;
}
h2 {
  font-weight: 500;
  font-size: 21px;
  letter-spacing: 0.12em;
}
.soft {
  color: var(--ink-soft);
  font-size: 12px;
  line-height: 1.8;
}
.tools {
  display: flex;
  gap: 10px;
  align-items: center;
  flex-wrap: wrap;
}
.search {
  display: flex;
  gap: 6px;
  align-items: center;
  border-bottom: 1px solid var(--ink);
  color: var(--ink-soft);
}
.search input {
  border: none;
  background: transparent;
  padding: 6px 0;
  font-size: 13px;
  width: 220px;
}
.search input:focus {
  outline: none;
}
select {
  border: 1px solid var(--line);
  background: var(--paper);
  padding: 7px 8px;
  font-size: 12px;
}
.err {
  color: var(--dot);
  font-size: 12px;
  margin-top: 12px;
}
table {
  width: 100%;
  border-collapse: collapse;
  margin-top: 16px;
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
  padding: 9px 8px;
  border-bottom: 1px solid var(--line);
  vertical-align: top;
}
.t {
  white-space: nowrap;
  font-family: var(--latin);
  font-size: 14px;
}
.d {
  word-break: break-all;
  font-size: 12px;
}
.act {
  display: inline-block;
  font-size: 11px;
  padding: 2px 9px;
  border-radius: 999px;
  border: 1px solid var(--line);
  white-space: nowrap;
}
.act.ans {
  border-color: var(--accent);
  color: var(--accent);
}
.act.dl {
  border-color: var(--dot);
  color: var(--dot);
}
.empty {
  text-align: center;
  padding: 30px;
}
@media (max-width: 760px) {
  .log {
    padding: 18px 14px 40px;
  }
  .search input {
    width: 160px;
  }
  th:nth-child(4),
  td:nth-child(4) {
    display: none;
  }
}
</style>
