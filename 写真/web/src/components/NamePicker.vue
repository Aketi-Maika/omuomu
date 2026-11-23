<script setup lang="ts">
// 名簿から「写っている人」を選ぶ欄。不明/退会/教職員は名簿に関係なく常に表示する
import { computed, ref } from 'vue'
import { FIXED, type FixedCounts, type FixedKey, type Person } from '../lib/types'

const props = defineProps<{ roster: Person[]; groups: string[] }>()
const selected = defineModel<string[]>('selected', { required: true })
const fixed = defineModel<FixedCounts>('fixed', { required: true })

const q = ref('')
const tab = ref<string>('')

const byId = computed(() => new Map(props.roster.map((p) => [p.id, p])))

function norm(s: string) {
  return s.normalize('NFKC').replace(/\s+/g, '').toLowerCase()
}

const filtered = computed(() => {
  const nq = norm(q.value)
  return props.roster.filter((p) => (!tab.value || p.group === tab.value) && (!nq || norm(p.name).includes(nq)))
})

const sections = computed(() => {
  const groups = [...props.groups]
  if (props.roster.some((p) => !p.group)) groups.push('')
  return groups
    .map((g) => ({ group: g, people: filtered.value.filter((p) => p.group === g) }))
    .filter((s) => s.people.length)
})

const total = computed(() => selected.value.length + FIXED.reduce((a, f) => a + fixed.value[f.key], 0))

function toggle(id: string) {
  const s = new Set(selected.value)
  s.has(id) ? s.delete(id) : s.add(id)
  selected.value = [...s]
}
function bump(key: FixedKey, d: number) {
  fixed.value = { ...fixed.value, [key]: Math.max(0, Math.min(99, fixed.value[key] + d)) }
}
function toggleFixed(key: FixedKey) {
  bump(key, fixed.value[key] > 0 ? -fixed.value[key] : 1)
}
function onSearchEnter() {
  if (filtered.value.length === 1) {
    toggle(filtered.value[0].id)
    q.value = ''
  }
}
</script>

<template>
  <div class="picker">
    <div class="label">選んだ人 ― {{ total }}名</div>
    <div class="sel" aria-live="polite">
      <button v-for="id in selected" :key="id" class="chip on" @click="toggle(id)">{{ byId.get(id)?.name ?? id }}</button>
      <template v-for="f in FIXED" :key="f.key">
        <button v-if="fixed[f.key] > 0" class="chip on" @click="bump(f.key, -fixed[f.key])">{{ f.label }} ×{{ fixed[f.key] }}</button>
      </template>
      <span v-if="!total" class="empty">まだ選ばれていません</span>
    </div>

    <div class="gl">名簿にいない人（いつでも選べます）</div>
    <div class="fixed-row">
      <span v-for="f in FIXED" :key="f.key" class="chip fx" :class="{ on: fixed[f.key] > 0 }">
        <button class="fx-name" :aria-pressed="fixed[f.key] > 0" @click="toggleFixed(f.key)">{{ f.label }}</button>
        <span class="cnt">
          <button :aria-label="`${f.label}を減らす`" @click="bump(f.key, -1)">−</button>
          <b>{{ fixed[f.key] }}</b>
          <button :aria-label="`${f.label}を増やす`" @click="bump(f.key, 1)">+</button>
        </span>
      </span>
    </div>

    <label class="search">
      <span aria-hidden="true">⌕</span>
      <input v-model="q" type="search" placeholder="名前で検索" enterkeyhint="done" @keydown.enter.prevent="onSearchEnter" />
      <span class="count">全 {{ roster.length }} 名</span>
    </label>
    <div v-if="groups.length > 1" class="tabs" role="tablist">
      <button role="tab" :class="{ on: tab === '' }" :aria-selected="tab === ''" @click="tab = ''">すべて</button>
      <button v-for="g in groups" :key="g" role="tab" :class="{ on: tab === g }" :aria-selected="tab === g" @click="tab = g">{{ g }}</button>
    </div>

    <div class="groups">
      <div v-for="s in sections" :key="s.group" class="g">
        <div class="gl">{{ s.group || 'その他' }}</div>
        <div class="list">
          <button
            v-for="p in s.people"
            :key="p.id"
            class="chip"
            :class="{ on: selected.includes(p.id) }"
            :aria-pressed="selected.includes(p.id)"
            @click="toggle(p.id)"
          >
            {{ p.name }}
          </button>
        </div>
      </div>
      <p v-if="!sections.length" class="empty">該当する人がいません</p>
    </div>
  </div>
</template>

<style scoped>
.sel {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin: 8px 0 16px;
  min-height: 36px;
}
.empty {
  font-size: 12px;
  color: var(--ink-soft);
  align-self: center;
}
.gl {
  font-size: 10px;
  letter-spacing: 0.3em;
  color: var(--ink-soft);
  margin-bottom: 7px;
}
.fixed-row {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
  padding: 10px 12px;
  background: var(--fixed-bg);
  border: 1px dashed var(--fixed-line);
  border-radius: 10px;
  margin-bottom: 16px;
}
.chip.fx {
  padding: 0;
  gap: 0;
  overflow: hidden;
}
.chip.fx.on::before {
  content: none;
}
.fx-name {
  padding: 7px 10px 7px 14px;
  font-size: 13px;
}
.chip.fx.on .fx-name::before {
  content: '✓ ';
  font-size: 11px;
}
.cnt {
  display: inline-flex;
  align-items: center;
  border-left: 1px solid color-mix(in srgb, currentColor 25%, transparent);
  font-size: 13px;
}
.cnt button {
  padding: 7px 9px;
  line-height: 1;
}
.cnt b {
  font-weight: 400;
  min-width: 1em;
  text-align: center;
}
.chip.fx:not(.on) .cnt {
  color: var(--ink-soft);
}
.search {
  display: flex;
  align-items: center;
  gap: 8px;
  border-bottom: 1px solid var(--ink);
  padding: 4px 2px;
  font-size: 13px;
  color: var(--ink-soft);
}
.search input {
  flex: 1;
  border: none;
  background: transparent;
  padding: 6px 0;
  font-size: 14px;
  color: var(--ink);
  min-width: 0;
}
.search input:focus {
  outline: none;
}
.count {
  font-size: 11px;
  white-space: nowrap;
}
.tabs {
  display: flex;
  gap: 16px;
  font-size: 12px;
  letter-spacing: 0.12em;
  color: var(--ink-soft);
  margin: 14px 0 12px;
  overflow-x: auto;
  scrollbar-width: none;
}
.tabs button {
  white-space: nowrap;
  padding-bottom: 3px;
  border-bottom: 1px solid transparent;
  letter-spacing: inherit;
  font-size: inherit;
}
.tabs .on {
  color: var(--ink);
  border-bottom-color: var(--ink);
}
.groups {
  margin-top: 12px;
}
.g {
  margin-bottom: 14px;
}
.list {
  display: flex;
  flex-wrap: wrap;
  gap: 7px;
}
</style>
