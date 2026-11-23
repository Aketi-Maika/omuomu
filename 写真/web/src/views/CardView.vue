<script setup lang="ts">
// フラッシュカード画面
// PC: 左=進捗/使い方/履歴、中央=写真カード、右=名簿から選ぶ欄
// スマホ: STEP1 写真 →「次へ」→ STEP2 名前を選ぶ → 記録して次の写真へ
import { computed, inject, onBeforeUnmount, onMounted, reactive, ref } from 'vue'
import { useRouter } from 'vue-router'
import PhotoCard from '../components/PhotoCard.vue'
import NamePicker from '../components/NamePicker.vue'
import EncImg from '../components/EncImg.vue'
import { SessionExpired, fetchStats, isMock, sessionId, submitResponse } from '../lib/api'
import { googleSignOut } from '../lib/auth'
import { drawNext } from '../lib/draw'
import { preload } from '../lib/images'
import { logout, store } from '../lib/store'
import { FIXED, type FixedCounts, type Photo } from '../lib/types'

const router = useRouter()
const toast = inject<(m: string) => void>('toast')!
const m = store.manifest!

const current = ref<Photo | null>(null)
const upcoming = ref<string | null>(null)
const selected = ref<string[]>([])
const fixed = ref<FixedCounts>({ unknown: 0, left: 0, staff: 0 })
const step = ref<'photo' | 'pick'>('photo')
const saving = ref(false)
const seen = reactive(new Set<string>())
const history = ref<{ id: string; note: string }[]>([])

const mq = window.matchMedia('(max-width: 760px)')
const mobile = ref(mq.matches)
const onMq = () => (mobile.value = mq.matches)

const indexOf = computed(() => new Map(m.photos.map((p, i) => [p.id, i + 1])))
const answers = computed(() => (current.value ? store.stats[current.value.id]?.n ?? 0 : 0))
const fullCount = computed(() => m.photos.filter((p) => (store.stats[p.id]?.n ?? 0) >= m.threshold).length)
const total = computed(() => selected.value.length + FIXED.reduce((a, f) => a + fixed.value[f.key], 0))
const nameOf = (id: string) => m.roster.find((p) => p.id === id)?.name ?? id

async function next() {
  const id = upcoming.value ?? (await drawNext(m, store.stats, seen, current.value?.id))
  upcoming.value = null
  current.value = m.photos.find((p) => p.id === id) ?? null
  selected.value = []
  fixed.value = { unknown: 0, left: 0, staff: 0 }
  step.value = 'photo'
  // 次の 1 枚を先に決めて読み込んでおく
  if (current.value) {
    const after = await drawNext(m, store.stats, new Set([...seen, current.value.id]), current.value.id)
    if (after && after !== current.value.id) {
      upcoming.value = after
      preload(after)
    }
  }
}

function pushHistory(note: string) {
  if (!current.value) return
  history.value = [{ id: current.value.id, note }, ...history.value.filter((h) => h.id !== current.value!.id)].slice(0, 3)
}

function skip() {
  if (!current.value) return
  seen.add(current.value.id)
  pushHistory('スキップ')
  next()
}

async function record() {
  if (!current.value || saving.value) return
  if (total.value === 0) {
    toast('写っている人を選んでください（分からない人は「不明」）')
    if (mobile.value) step.value = 'pick'
    return
  }
  saving.value = true
  const p = current.value
  try {
    await submitResponse({
      photoId: p.id,
      photoTitle: p.title,
      people: selected.value,
      names: selected.value.map(nameOf),
      unknown: fixed.value.unknown,
      left: fixed.value.left,
      staff: fixed.value.staff,
      sessionId: sessionId(),
    })
    seen.add(p.id)
    pushHistory(`写っている人 ${total.value}名`)
    toast(isMock() ? '記録しました（確認モード：この端末にだけ保存）' : '記録しました')
    next()
  } catch (e) {
    console.error(e)
    if (e instanceof SessionExpired) {
      toast('ログインの有効期限が切れました。もう一度ログインしてください')
      router.push('/')
      return
    }
    toast('保存できませんでした: ' + ((e as Error).message || '通信状況を確かめてください'))
  } finally {
    saving.value = false
  }
}

function onKey(e: KeyboardEvent) {
  const t = e.target as HTMLElement
  if (t.closest('input, textarea, [role="dialog"]')) return
  if (e.key === 'Enter' || e.key === 'ArrowRight') {
    e.preventDefault()
    mobile.value && step.value === 'photo' ? (step.value = 'pick') : record()
  } else if (e.key === 'ArrowLeft') {
    e.preventDefault()
    skip()
  }
}

// スマホ: カードを左にスワイプ=スキップ、右にスワイプ=名前を選ぶ
let sx = 0
let sy = 0
function onTouchStart(e: TouchEvent) {
  sx = e.touches[0].clientX
  sy = e.touches[0].clientY
}
function onTouchEnd(e: TouchEvent) {
  const dx = e.changedTouches[0].clientX - sx
  const dy = e.changedTouches[0].clientY - sy
  if (Math.abs(dx) < 70 || Math.abs(dy) > Math.abs(dx)) return
  dx < 0 ? skip() : (step.value = 'pick')
}

function leave() {
  logout()
  googleSignOut()
  router.push('/')
}

onMounted(async () => {
  mq.addEventListener('change', onMq)
  window.addEventListener('keydown', onKey)
  try {
    await fetchStats()
  } catch {
    router.push('/')
    return
  }
  if (store.statsError) toast(store.statsError)
  await next()
})
onBeforeUnmount(() => {
  mq.removeEventListener('change', onMq)
  window.removeEventListener('keydown', onKey)
})
</script>

<template>
  <header class="top">
    <div class="brand">{{ m.title }}<small>Portraits of Us</small></div>
    <nav class="nav">
      <span class="on">カード</span>
      <RouterLink v-if="store.user?.role === 'admin'" to="/admin">管理室</RouterLink>
      <span class="who" :title="store.user?.email">{{ store.user?.name }}</span>
      <button @click="leave">退出</button>
    </nav>
    <div class="mbar" aria-hidden="true"><i :style="{ width: (fullCount / m.photos.length) * 100 + '%' }"></i></div>
  </header>

  <main class="room" :class="[mobile ? 'is-mobile' : 'is-pc', 'step-' + step]">
    <!-- 左: 進捗・使い方・履歴（PC のみ） -->
    <aside class="side">
      <div class="label">回答が{{ m.threshold }}件以上の作品</div>
      <div class="counter latin">
        {{ fullCount }} <small>/ {{ m.photos.length }}</small>
      </div>
      <div class="bar"><i :style="{ width: (fullCount / m.photos.length) * 100 + '%' }"></i></div>
      <div class="label mt">HOW TO</div>
      <ol class="how">
        <li>カードが無作為に配られます</li>
        <li>右の名簿から写っている人を選ぶ</li>
        <li>記録すると次のカードへ</li>
        <li class="soft">回答が{{ m.threshold }}件集まった作品は<br />めったに出なくなります</li>
      </ol>
      <div class="keys">
        <kbd>Enter</kbd> 記録して次へ<br /><kbd>←</kbd> スキップ
      </div>
      <template v-if="history.length">
        <div class="label mt">さきほどのカード</div>
        <div v-for="h in history" :key="h.id" class="hist">
          <div class="hthumb"><EncImg :id="h.id" thumb /></div>
          <div>
            <b class="latin">No.{{ String(indexOf.get(h.id)).padStart(3, '0') }}</b><br />{{ h.note }}
          </div>
        </div>
      </template>
      <p v-if="isMock()" class="mock">確認モード：回答はこの端末にだけ保存されます</p>
      <p v-else class="signed">{{ store.user?.email }} として記録します</p>
    </aside>

    <!-- 中央: カード -->
    <section class="stage" @touchstart.passive="onTouchStart" @touchend.passive="onTouchEnd">
      <div class="card-wrap">
        <PhotoCard v-if="current" :photo="current" :no="indexOf.get(current.id) ?? 0" :answers="answers" :threshold="m.threshold" />
        <p v-else class="loading">カードを配っています…</p>
      </div>
      <div v-if="!mobile" class="ctrl">
        <div><button class="round" aria-label="スキップ" @click="skip">←</button><div class="ctrl-l">スキップ</div></div>
        <div><button class="round" aria-label="記録して次へ" @click="record">→</button><div class="ctrl-l">記録して次へ</div></div>
      </div>
      <template v-else>
        <p class="hint">STEP 1 / 2 ・ 写真をよく見てください</p>
        <div class="mctrl">
          <button class="btn" @click="skip">スキップ</button>
          <button class="btn fill grow" @click="step = 'pick'">次へ ― 名前を選ぶ →</button>
        </div>
      </template>
    </section>

    <!-- 右: 名簿から選ぶ（スマホでは STEP 2 の画面） -->
    <section class="names" :aria-hidden="mobile && step !== 'pick'">
      <div class="nh">
        <div v-if="mobile && current" class="nthumb"><EncImg :id="current.id" thumb /></div>
        <div>
          <div class="label">
            {{ mobile ? 'STEP 2 / 2' : '名簿から選ぶ' }} ・ No. {{ String(current ? indexOf.get(current.id) : 0).padStart(3, '0') }}
          </div>
          <div class="q serif">写っているのは、だれ？</div>
        </div>
      </div>
      <div class="nb">
        <NamePicker v-model:selected="selected" v-model:fixed="fixed" :roster="m.roster" :groups="m.groups" />
      </div>
      <div class="nf">
        <button v-if="mobile" class="sub" @click="step = 'photo'">← 写真に戻る</button>
        <button v-else class="sub" @click="((selected = []), (fixed = { unknown: 0, left: 0, staff: 0 }))">選択をクリア</button>
        <button class="btn fill" :disabled="saving" @click="record">
          {{ saving ? '記録しています…' : mobile ? '記録して次の写真へ' : '記録して次へ →' }}
        </button>
      </div>
    </section>
  </main>
</template>

<style scoped>
.top {
  position: sticky;
}
.mbar {
  display: none;
}

.room {
  display: grid;
  grid-template-columns: 230px minmax(320px, 480px) minmax(360px, 560px);
  justify-content: center;
  gap: 40px;
  padding: 36px 40px 40px;
  min-height: calc(100dvh - var(--top-h));
  align-items: start;
}
.side {
  padding-top: 10px;
  font-size: 12px;
}
.counter {
  font-size: 38px;
  margin-top: 4px;
}
.counter small {
  font-size: 14px;
  color: var(--ink-soft);
}
.bar {
  width: 200px;
  height: 2px;
  background: var(--line);
  margin-top: 8px;
}
.bar i {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width 0.4s;
}
.mt {
  margin-top: 32px;
}
.how {
  margin: 12px 0 0 18px;
  line-height: 2.1;
  letter-spacing: 0.04em;
}
.soft {
  color: var(--ink-soft);
}
.keys {
  margin-top: 16px;
  font-size: 11px;
  color: var(--ink-soft);
  line-height: 2.4;
}
kbd {
  border: 1px solid var(--line);
  background: var(--paper);
  padding: 2px 7px;
  font-family: ui-monospace, monospace;
  color: var(--ink);
}
.hist {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-top: 12px;
  font-size: 11px;
  color: var(--ink-soft);
  line-height: 1.6;
}
.hthumb {
  width: 58px;
  height: 42px;
  border: 3px solid var(--frame);
  flex: none;
  background: var(--mat);
}
.hthumb :deep(img) {
  object-fit: cover;
}
.hist b {
  font-size: 14px;
  color: var(--ink);
  font-weight: 400;
}
.signed {
  margin-top: 28px;
  font-size: 11px;
  color: var(--ink-soft);
  word-break: break-all;
}
.who {
  max-width: 16em;
  overflow: hidden;
  text-overflow: ellipsis;
}
.mock {
  margin-top: 28px;
  font-size: 11px;
  color: var(--dot);
  line-height: 1.7;
}

.stage {
  position: relative;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 20px;
}
.stage::before {
  content: '';
  position: absolute;
  left: 50%;
  top: -120px;
  width: 150%;
  height: 120%;
  transform: translateX(-50%);
  background: radial-gradient(ellipse 50% 55% at 50% 30%, var(--spot), transparent 70%);
  pointer-events: none;
}
.card-wrap {
  position: relative;
  width: 100%;
  height: min(600px, calc(100dvh - var(--top-h) - 170px));
  min-height: 420px;
}
.loading {
  padding-top: 40%;
  text-align: center;
  color: var(--ink-soft);
  letter-spacing: 0.2em;
  font-size: 13px;
}
.ctrl {
  display: flex;
  gap: 22px;
  position: relative;
}

.names {
  background: var(--paper);
  border: 1px solid var(--line);
  border-radius: 14px;
  box-shadow: 0 2px 3px rgba(0, 0, 0, 0.05), 0 30px 60px -24px var(--shadow);
  display: flex;
  flex-direction: column;
  height: min(600px, calc(100dvh - var(--top-h) - 170px));
  min-height: 420px;
  overflow: hidden;
}
.nh {
  display: flex;
  gap: 14px;
  align-items: center;
  padding: 20px 24px 14px;
  border-bottom: 1px solid var(--line);
}
.q {
  font-size: 19px;
  font-weight: 500;
  letter-spacing: 0.1em;
  margin-top: 4px;
}
.nb {
  padding: 16px 24px;
  flex: 1;
  overflow-y: auto;
  overscroll-behavior: contain;
}
.nf {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  padding: 14px 24px 18px;
  border-top: 1px solid var(--line);
}
.sub {
  font-size: 12px;
  letter-spacing: 0.15em;
  color: var(--ink-soft);
}


/* 中くらいの画面: 左の欄を省略 */
@media (max-width: 1180px) {
  .room {
    grid-template-columns: minmax(300px, 460px) minmax(340px, 520px);
    gap: 28px;
    padding: 28px 24px;
  }
  .side {
    display: none;
  }
}

/* スマホ: 2 ステップ */
.room.is-mobile {
  display: block;
  padding: 0;
  position: relative;
  overflow: hidden;
  min-height: calc(100dvh - var(--top-h) - env(safe-area-inset-top));
}
.is-mobile .side {
  display: none;
}
.is-mobile .stage {
  padding: 22px 18px calc(18px + env(safe-area-inset-bottom));
  height: calc(100dvh - var(--top-h) - env(safe-area-inset-top));
  gap: 12px;
  transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
}
.is-mobile .card-wrap {
  display: flex;
  flex: 1;
  height: auto;
  min-height: 0;
}
.is-mobile .hint {
  font-size: 10px;
  letter-spacing: 0.25em;
  color: var(--ink-soft);
}
.mctrl {
  display: flex;
  gap: 10px;
  width: 100%;
}
.mctrl .btn {
  padding: 16px 18px;
  letter-spacing: 0.12em;
}
.grow {
  flex: 1;
}
.is-mobile .names {
  position: absolute;
  inset: 12px 12px calc(12px + env(safe-area-inset-bottom));
  height: auto;
  min-height: 0;
  transform: translateX(110%);
  transition: transform 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  z-index: 5;
}
.is-mobile.step-pick .names {
  transform: none;
}
.is-mobile.step-pick .stage {
  transform: translateX(-30%);
  opacity: 0.4;
}
.is-mobile .nh {
  padding: 14px 16px 12px;
}
.is-mobile .q {
  font-size: 16px;
}
.nthumb {
  width: 58px;
  height: 44px;
  border: 3px solid var(--frame);
  flex: none;
  background: var(--mat);
}
.nthumb :deep(img) {
  object-fit: cover;
}
.is-mobile .nb {
  padding: 12px 16px;
}
.is-mobile .nf {
  padding: 12px 16px 14px;
}
.is-mobile .nf .btn {
  padding: 13px 18px;
  letter-spacing: 0.12em;
}
.is-mobile ~ * .mbar,
.top .mbar {
  display: none;
}
@media (max-width: 760px) {
  .top .mbar {
    display: block;
    position: absolute;
    left: 0;
    right: 0;
    bottom: -1px;
    height: 2px;
    background: var(--line);
  }
  .mbar i {
    display: block;
    height: 100%;
    background: var(--accent);
  }
  .nav .on {
    display: none;
  }
}
</style>
