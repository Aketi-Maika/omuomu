<script setup lang="ts">
import { provide, ref } from 'vue'

const toastMsg = ref('')
let timer: number | undefined
function toast(msg: string, ms = 2600) {
  toastMsg.value = msg
  clearTimeout(timer)
  timer = window.setTimeout(() => (toastMsg.value = ''), ms)
}
provide('toast', toast)
</script>

<template>
  <div class="wall-bg" aria-hidden="true"></div>
  <RouterView />
  <Transition name="toast">
    <div v-if="toastMsg" class="toast" role="status">{{ toastMsg }}</div>
  </Transition>
</template>
