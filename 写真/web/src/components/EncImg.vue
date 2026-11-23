<script setup lang="ts">
// 暗号化された写真を、画面に入ったときに復号して表示する
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { imageUrl } from '../lib/images'

const props = defineProps<{ id: string; thumb?: boolean; alt?: string; eager?: boolean }>()
const src = ref('')
const failed = ref(false)
const el = ref<HTMLElement>()
let io: IntersectionObserver | null = null

async function load() {
  failed.value = false
  const id = props.id
  try {
    const u = await imageUrl(id, props.thumb)
    if (id === props.id) src.value = u
  } catch {
    failed.value = true
  }
}

onMounted(() => {
  if (props.eager || !('IntersectionObserver' in window)) return void load()
  io = new IntersectionObserver(
    (es) => {
      if (es.some((e) => e.isIntersecting)) {
        io?.disconnect()
        load()
      }
    },
    { rootMargin: '300px' },
  )
  io.observe(el.value!)
})
onBeforeUnmount(() => io?.disconnect())
watch(
  () => [props.id, props.thumb],
  () => {
    src.value = ''
    load()
  },
)
</script>

<template>
  <div ref="el" class="enc" :class="{ loaded: !!src }">
    <img v-if="src" :src="src" :alt="alt ?? ''" draggable="false" />
    <span v-else-if="failed" class="err">表示できません</span>
  </div>
</template>

<style scoped>
.enc {
  position: relative;
  width: 100%;
  height: 100%;
  background: linear-gradient(110deg, color-mix(in srgb, var(--line) 50%, transparent) 30%, color-mix(in srgb, var(--line) 20%, transparent) 50%, color-mix(in srgb, var(--line) 50%, transparent) 70%);
  background-size: 300% 100%;
  animation: shimmer 1.6s linear infinite;
  overflow: hidden;
}
.enc.loaded {
  animation: none;
  background: none;
}
img {
  display: block;
  width: 100%;
  height: 100%;
  object-fit: contain;
  animation: fade 0.5s ease;
  -webkit-user-drag: none;
}
.err {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  color: var(--ink-soft);
}
@keyframes shimmer {
  from {
    background-position: 100% 0;
  }
  to {
    background-position: -200% 0;
  }
}
@keyframes fade {
  from {
    opacity: 0;
  }
}
</style>
