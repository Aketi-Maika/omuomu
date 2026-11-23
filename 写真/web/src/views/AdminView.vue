<script setup lang="ts">
// 管理室: 人物で探す（ZIP）/ 入力状況 / ログ / 設定（PC のローカル管理ツールで開いたときのみ）
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import SearchPanel from '../components/admin/SearchPanel.vue'
import StatusPanel from '../components/admin/StatusPanel.vue'
import SettingsPanel from '../components/admin/SettingsPanel.vue'
import LogPanel from '../components/admin/LogPanel.vue'
import { fetchStats, logEvent } from '../lib/api'
import { renderGoogleButton, signInLocalPreview, signOut } from '../lib/auth'
import { loadPublic, restoreSession, store } from '../lib/store'
import { localStatus, type LocalStatus } from '../lib/local'

const router = useRouter()
const local = ref<LocalStatus | null>(null)
const checking = ref(true)
const tab = ref<'search' | 'status' | 'log' | 'settings'>('search')
const busy = ref(false)
const error = ref('')
const gbtn = ref<HTMLElement>()

const authed = computed(() => !!store.manifest && store.user?.role === 'admin')
const viewerOnly = computed(() => !!store.user && store.user.role !== 'admin')
const hasData = computed(() => !!store.pub)

async function afterLogin() {
  if (!authed.value) return
  logEvent('管理室に入室')
  fetchStats().catch(() => router.push('/'))
}

onMounted(async () => {
  local.value = await localStatus()
  await restoreSession()
  checking.value = false
  if (local.value && !hasData.value) tab.value = 'settings'
  if (authed.value) afterLogin()
  else if (hasData.value && !store.user) {
    await nextTick()
    if (gbtn.value)
      renderGoogleButton(gbtn.value, {
        onStart: () => ((busy.value = true), (error.value = '')),
        onDone: () => ((busy.value = false), afterLogin()),
        onError: (m) => ((busy.value = false), (error.value = local.value && !store.pub?.googleClientId ? '' : m)),
      }).catch((e) => (error.value = e.message))
  }
})

watch(tab, (t) => {
  if (authed.value && t !== 'settings') logEvent('管理室のタブ', { tab: t })
})

async function enterLocal() {
  busy.value = true
  try {
    if (await signInLocalPreview()) afterLogin()
  } finally {
    busy.value = false
  }
}

function leave() {
  signOut()
  router.push('/')
}
</script>

<template>
  <header class="top admin-top">
    <div class="brand">管理室<small>Curator's Room</small></div>
    <nav class="nav">
      <template v-if="authed">
        <button :class="{ on: tab === 'search' }" @click="tab = 'search'">人物で探す</button>
        <button :class="{ on: tab === 'status' }" @click="tab = 'status'">入力状況</button>
        <button :class="{ on: tab === 'log' }" @click="tab = 'log'">ログ</button>
      </template>
      <button v-if="local" :class="{ on: tab === 'settings' }" @click="tab = 'settings'">設定</button>
      <span v-if="store.user" class="who" :title="store.user.email">{{ store.user.name }}</span>
      <button v-if="store.user" @click="leave">退出</button>
      <RouterLink v-else to="/">入口へ</RouterLink>
    </nav>
  </header>

  <main class="admin">
    <p v-if="checking" class="center soft">読み込み中…</p>

    <SettingsPanel v-else-if="tab === 'settings' && local" :local="local" @built="loadPublic(true)" />

    <section v-else-if="!authed" class="login">
      <template v-if="viewerOnly">
        <h1 class="serif">管理者として登録されていません</h1>
        <p class="soft">{{ store.user?.email }} は閲覧のみできます。管理者の追加は、PC の管理ツールの「設定」で行います。</p>
        <RouterLink to="/cards" class="btn mt">カードに戻る</RouterLink>
      </template>
      <template v-else-if="hasData">
        <h1 class="serif">管理室に入る</h1>
        <p class="soft">管理者として登録された Google アカウントでログインしてください。操作はアカウントごとに記録されます。</p>
        <div ref="gbtn" class="gbtn" :class="{ busy }"></div>
        <p class="err" aria-live="polite">{{ busy ? '確認しています…' : error }}</p>
        <button v-if="local" class="local-btn" :disabled="busy" @click="enterLocal">管理ツールで確認（Google ログインなし）</button>
      </template>
      <template v-else>
        <h1 class="serif">まだ写真が公開されていません</h1>
        <p class="soft">NAS に届く PC で管理ツール（omuomu-gallery）を起動し、「設定」から写真フォルダーを登録して構築してください。</p>
      </template>
    </section>

    <SearchPanel v-else-if="tab === 'search'" :local="local" />
    <StatusPanel v-else-if="tab === 'status'" />
    <LogPanel v-else-if="tab === 'log'" />
  </main>
</template>

<style scoped>
.admin-top {
  background: #2b2622;
  color: #efe9df;
  border-bottom: none;
}
.admin-top .brand small,
.admin-top .nav {
  color: #a89f93;
}
.admin-top .nav .on {
  color: #fff;
  border-color: #fff;
}
.who {
  max-width: 14em;
  overflow: hidden;
  text-overflow: ellipsis;
}
.admin {
  min-height: calc(100dvh - var(--top-h));
  background: var(--paper-2);
}
.center {
  text-align: center;
  padding: 80px 0;
}
.soft {
  color: var(--ink-soft);
  font-size: 13px;
  line-height: 1.8;
}
.login {
  max-width: 480px;
  margin: 0 auto;
  padding: 14vh 24px;
}
.login h1 {
  font-weight: 500;
  font-size: 24px;
  letter-spacing: 0.12em;
  margin-bottom: 10px;
}
.gbtn {
  margin-top: 24px;
  min-height: 44px;
}
.gbtn.busy {
  opacity: 0.5;
  pointer-events: none;
}
.mt {
  margin-top: 24px;
  text-decoration: none;
}
.err {
  color: var(--dot);
  font-size: 12px;
  margin-top: 12px;
  min-height: 1em;
}
.local-btn {
  margin-top: 18px;
  font-size: 12px;
  color: var(--accent);
  border-bottom: 1px dashed var(--accent);
  padding-bottom: 2px;
}
@media (max-width: 760px) {
  .who {
    display: none;
  }
}
</style>
