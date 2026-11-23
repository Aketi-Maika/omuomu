<script setup lang="ts">
// 山札の一番上のカード（額装された写真）
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import EncImg from './EncImg.vue'
import type { Photo } from '../lib/types'

const props = defineProps<{ photo: Photo; no: number; answers: number; threshold: number }>()
const filled = computed(() => Math.min(props.threshold, props.answers))

// 写真の縦横比に合わせてカードの大きさを決める（余白を出さない）
const box = ref<HTMLElement>()
const size = ref({ w: 0, h: 0 })
let ro: ResizeObserver | null = null
onMounted(() => {
  ro = new ResizeObserver(([e]) => (size.value = { w: e.contentRect.width, h: e.contentRect.height }))
  ro.observe(box.value!)
})
onBeforeUnmount(() => ro?.disconnect())
// カードの枠・余白・下の作品名の分
const CHROME_W = 70
const CHROME_H = 122
const picStyle = computed(() => {
  const { w, h } = size.value
  if (!w || !h) return {}
  const s = Math.min((w - CHROME_W) / props.photo.w, (h - CHROME_H) / props.photo.h)
  return { width: Math.max(120, props.photo.w * s) + 'px', height: Math.max(90, props.photo.h * s) + 'px' }
})
</script>

<template>
  <div ref="box" class="box">
  <div class="deck">
    <div class="ghost g1"></div>
    <div class="ghost g2"></div>
    <Transition name="deal" mode="out-in">
      <article :key="photo.id" class="card">
        <div class="corner" :title="`回答 ${answers} 件`">
          <template v-if="answers === 0"><span class="dot"></span>未回答</template>
          <template v-else>
            回答 {{ answers }}<span class="of">/ {{ threshold }}</span>
            <span class="meter" aria-hidden="true">
              <i v-for="i in threshold" :key="i" :class="{ f: i <= filled }"></i>
            </span>
          </template>
        </div>
        <div class="pic">
          <div class="inner"><div class="img" :style="picStyle"><EncImg :id="photo.id" eager :alt="photo.title" /></div></div>
        </div>
        <div class="meta">
          <div>
            <div class="no latin">No. {{ String(no).padStart(3, '0') }}</div>
            <div class="t serif">{{ photo.title }}</div>
          </div>
          <div class="s">{{ photo.folder }}</div>
        </div>
      </article>
    </Transition>
  </div>
  </div>
</template>

<style scoped>
.box {
  flex: 1;
  align-self: stretch;
  width: 100%;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.deck {
  position: relative;
}
.ghost {
  position: absolute;
  inset: 0;
  background: var(--paper-2);
  border-radius: 14px;
  border: 1px solid var(--line);
  box-shadow: 0 10px 30px -14px var(--shadow);
}
.g1 {
  transform: rotate(-3.2deg) translate(-8px, 6px);
}
.g2 {
  transform: rotate(2.4deg) translate(10px, 4px);
}
.card {
  position: relative;
  z-index: 2;
  background: var(--paper);
  border-radius: 14px;
  border: 1px solid var(--line);
  box-shadow: 0 2px 3px rgba(0, 0, 0, 0.06), 0 30px 60px -20px var(--shadow);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}
.pic {
  margin: 16px 16px 0;
  background: var(--frame);
  padding: 7px;
  border-radius: 3px;
}
.inner {
  background: var(--mat);
  padding: 12px;
}
.img {
  width: 100%;
  aspect-ratio: 4 / 3;
}
.meta {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 12px;
  padding: 12px 20px 14px;
}
.no {
  font-size: 18px;
  letter-spacing: 0.1em;
}
.t {
  font-size: 14px;
  font-weight: 500;
  letter-spacing: 0.06em;
  word-break: break-all;
}
.s {
  font-size: 11px;
  color: var(--ink-soft);
  text-align: right;
}
.corner {
  position: absolute;
  top: 26px;
  right: 26px;
  z-index: 3;
  background: color-mix(in srgb, var(--paper) 92%, transparent);
  color: var(--ink);
  border-radius: 999px;
  padding: 4px 11px;
  font-size: 11px;
  letter-spacing: 0.08em;
  display: flex;
  align-items: center;
  gap: 4px;
}
.of {
  color: var(--ink-soft);
}
.meter {
  display: inline-flex;
  gap: 2px;
  margin-left: 4px;
}
.meter i {
  width: 4px;
  height: 9px;
  background: var(--line);
}
.meter i.f {
  background: var(--accent);
}

.deal-enter-active {
  transition: transform 0.45s cubic-bezier(0.2, 0.8, 0.2, 1), opacity 0.3s;
}
.deal-leave-active {
  transition: transform 0.3s ease-in, opacity 0.3s;
}
.deal-enter-from {
  transform: translateY(18px) rotate(-1.5deg) scale(0.97);
  opacity: 0;
}
.deal-leave-to {
  transform: translateX(-60px) rotate(-6deg);
  opacity: 0;
}
</style>
