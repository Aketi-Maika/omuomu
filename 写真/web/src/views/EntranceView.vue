<script setup lang="ts">
import { computed, nextTick, onMounted, ref } from 'vue'
import { useRouter } from 'vue-router'
import { loadPublic, store } from '../lib/store'
import { loadWasm } from '../lib/crypto'
import { googleUsableHere, renderGoogleButton, signInLocalPreview } from '../lib/auth'
import { localStatus } from '../lib/local'

const router = useRouter()
const busy = ref(false)
const error = ref('')
const gbtn = ref<HTMLElement>()
const isLocal = ref(false)
// 「、」「。」の後で改行できるように分ける（単語の途中で折り返さない）
const titleParts = computed(() => (store.pub?.title ?? '　').split(/(?<=[、。,])/))

onMounted(async () => {
  loadWasm()
  const [pub, local] = await Promise.all([loadPublic(), localStatus()])
  isLocal.value = !!local
  if (!pub) return
  await nextTick()
  renderGoogleButton(gbtn.value!, {
    onStart: () => {
      busy.value = true
      error.value = ''
    },
    onDone: () => {
      busy.value = false
      router.push('/cards')
    },
    onError: (m) => {
      busy.value = false
      // この PC で開いているときは、未設定でも下の「この PC で確認」から入れる
      error.value = isLocal.value && !store.pub?.googleClientId && googleUsableHere() ? '' : m
    },
  }).catch((e) => (error.value = e.message))
})

async function enterLocal() {
  busy.value = true
  try {
    if (await signInLocalPreview()) router.push('/cards')
    else error.value = 'この PC の管理ツールに接続できません'
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <main class="entrance">
    <section class="left">
      <div class="kicker">PHOTO EXHIBITION</div>
      <h1 class="serif">
        <span v-for="(part, i) in titleParts" :key="i" class="tp">{{ part }}</span>
      </h1>
      <div v-if="store.pub?.subtitle" class="en latin">{{ store.pub.subtitle }}</div>
      <div class="rule"></div>
      <dl class="meta">
        <template v-if="store.pub?.period"><dt>会期</dt><dd>{{ store.pub.period }}</dd></template>
        <template v-if="store.pub"><dt>出品数</dt><dd>{{ store.pub.count }} 点</dd></template>
        <template v-if="store.pub?.notice"><dt>ご案内</dt><dd>{{ store.pub.notice }}</dd></template>
      </dl>

      <p v-if="store.pubError" class="err">{{ store.pubError }}</p>
      <div v-else class="form">
        <div class="label">ENTRANCE</div>
        <div ref="gbtn" class="gbtn" :class="{ busy }" aria-label="Google でログイン"></div>
        <p class="hint" :class="{ err: !!error }" aria-live="polite">
          <template v-if="busy">確認しています…</template>
          <template v-else-if="error">{{ error }}</template>
          <template v-else>
            {{ store.pub?.allowedDomains.length ? `@${store.pub.allowedDomains.join(' / @')} の Google アカウントで入場できます。` : 'Google アカウントで入場します。' }}<br />
            入場・回答の記録には、ログインしたアカウントのメールアドレスが残ります。
          </template>
        </p>
        <button v-if="isLocal" class="local-btn" :disabled="busy" @click="enterLocal">管理ツールで確認（Google ログインなし・記録はこの端末だけ）</button>
      </div>
    </section>

    <section class="right" aria-hidden="true">
      <div class="spot">
        <div class="frame poster"><div class="mat"><div class="art"></div></div></div>
        <div class="caption">
          <div class="latin no">No. 000</div>
          <div class="serif">写っているのは、だれ？</div>
          <div class="m">展覧会のはじまり</div>
        </div>
      </div>
    </section>

    <div class="vertical serif" aria-hidden="true">写っている人を、さがす展覧会</div>
    <RouterLink to="/admin" class="admin-link">管理室</RouterLink>
  </main>
</template>

<style scoped>
.entrance {
  min-height: 100dvh;
  display: grid;
  grid-template-columns: 1fr 1fr;
  position: relative;
  padding: env(safe-area-inset-top) env(safe-area-inset-right) env(safe-area-inset-bottom) env(safe-area-inset-left);
}
.left {
  padding: 12vh 0 80px 8vw;
}
.kicker {
  font-size: 12px;
  letter-spacing: 0.5em;
  color: var(--ink-soft);
}
h1 {
  font-weight: 300;
  font-size: clamp(36px, 4.6vw, 64px);
  line-height: 1.35;
  letter-spacing: 0.08em;
  margin: 30px 0 14px;
}
.tp {
  display: inline-block;
}
.en {
  font-style: italic;
  font-size: clamp(17px, 2vw, 28px);
  color: var(--ink-soft);
}
.rule {
  width: 60px;
  height: 1px;
  background: var(--ink);
  margin: 40px 0 34px;
}
.meta {
  display: grid;
  grid-template-columns: 92px 1fr;
  row-gap: 10px;
  font-size: 13px;
  letter-spacing: 0.1em;
}
.meta dt {
  font-size: 11px;
  letter-spacing: 0.3em;
  color: var(--ink-soft);
  padding-top: 2px;
}
.form {
  margin-top: 50px;
  max-width: 400px;
}
.gbtn {
  margin-top: 12px;
  min-height: 44px;
}
.gbtn.busy {
  opacity: 0.5;
  pointer-events: none;
}
.local-btn {
  margin-top: 18px;
  font-size: 12px;
  letter-spacing: 0.06em;
  color: var(--accent);
  border-bottom: 1px dashed var(--accent);
  padding-bottom: 2px;
  text-align: left;
}
.hint {
  font-size: 11px;
  color: var(--ink-soft);
  margin-top: 14px;
  letter-spacing: 0.04em;
  line-height: 1.8;
}
.err {
  color: var(--dot);
  font-size: 13px;
  margin-top: 40px;
}
.hint.err {
  margin-top: 14px;
  font-size: 12px;
}
.right {
  display: flex;
  align-items: center;
  justify-content: center;
}
.spot {
  position: relative;
}
.spot::before {
  content: '';
  position: absolute;
  left: 50%;
  top: -90px;
  width: 140%;
  height: 150%;
  transform: translateX(-50%);
  background: radial-gradient(ellipse 50% 55% at 50% 30%, var(--spot), transparent 70%);
  pointer-events: none;
}
.poster {
  position: relative;
  width: min(30vw, 440px);
  aspect-ratio: 4 / 5;
}
.poster .mat {
  height: 100%;
  padding: 8%;
}
.art {
  height: 100%;
  background:
    radial-gradient(circle at 70% 34%, #ffd9a1 0 8%, transparent 8.6%),
    linear-gradient(180deg, #f3a86b 0%, #e9746a 40%, #5b3c5e 70%, #2b2238 100%);
}
.caption {
  position: relative;
  margin: 22px 0 0 auto;
  width: 200px;
  background: var(--paper);
  border: 1px solid var(--line);
  padding: 10px 14px;
  font-size: 12px;
  line-height: 1.7;
}
.caption .no {
  font-size: 15px;
}
.caption .m {
  color: var(--ink-soft);
  font-size: 11px;
}
.vertical {
  position: absolute;
  right: 40px;
  top: 100px;
  writing-mode: vertical-rl;
  font-size: 13px;
  letter-spacing: 0.6em;
  color: var(--ink-soft);
}
.admin-link {
  position: absolute;
  left: 8vw;
  bottom: calc(24px + env(safe-area-inset-bottom));
  font-size: 11px;
  letter-spacing: 0.3em;
  color: var(--ink-soft);
  text-decoration: none;
}

@media (max-width: 860px) {
  .entrance {
    grid-template-columns: 1fr;
    grid-template-rows: auto auto;
  }
  .left {
    padding: calc(48px + env(safe-area-inset-top)) 28px 0;
    display: contents;
  }
  .left > * {
    margin-left: 28px;
    margin-right: 28px;
  }
  .kicker {
    margin-top: 48px;
    font-size: 10px;
  }
  h1 {
    margin-top: 16px;
  }
  .rule,
  .meta {
    display: none;
  }
  .right {
    order: 0;
    padding: 24px 0 8px;
  }
  .poster {
    width: 58vw;
  }
  .caption {
    display: none;
  }
  .form {
    order: 1;
    margin-top: 28px;
    padding-bottom: 72px;
  }
  .vertical {
    display: none;
  }
  .admin-link {
    left: 28px;
  }
}
</style>
