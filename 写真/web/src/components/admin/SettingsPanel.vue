<script setup lang="ts">
// 設定（NAS の写真フォルダー・名簿・パスワードなど）→ 構築 → 公開
// PC で omuomu-gallery を起動して開いたときだけ使える
import { computed, inject, onBeforeUnmount, onMounted, ref } from 'vue'
import { local as api, type BuildStatus, type LocalConfig, type LocalStatus } from '../../lib/local'

const props = defineProps<{ local: LocalStatus | null }>()
const emit = defineEmits<{ built: [] }>()
const toast = inject<(m: string) => void>('toast')!

const cfg = ref<LocalConfig | null>(null)
const problems = ref<string[]>([])
const warnings = ref<string[]>([])
const adminText = ref('')
const blockedText = ref('')
const setupCode = ref('')
const apiCheck = ref('')
const mirrorMsg = ref('')
const dirty = ref(false)
const checks = ref<Record<string, string>>({})
const build = ref<BuildStatus | null>(null)
const publishing = ref(false)
const publishLog = ref<string[]>([])
const commitMsg = ref('写真展データを更新')

let timer: number | undefined

onMounted(async () => {
  if (!props.local) return
  cfg.value = await api.getConfig()
  if (!cfg.value.photoDirs.length) cfg.value.photoDirs.push('')
  adminText.value = cfg.value.adminEmails.join('\n')
  blockedText.value = cfg.value.blockedEmails.join('\n')
  if (!cfg.value.allowedOrigins.length && pagesOrigin.value) cfg.value.allowedOrigins = [pagesOrigin.value]
  build.value = await api.buildStatus().catch(() => null)
  if (build.value?.running) poll()
  cfg.value.photoDirs.forEach((_, i) => check('dir' + i, cfg.value!.photoDirs[i]))
  if (cfg.value.rosterPath) check('roster', cfg.value.rosterPath)
})
onBeforeUnmount(() => clearTimeout(timer))

function touch() {
  dirty.value = true
}

async function check(key: string, p: string) {
  if (!p.trim()) {
    checks.value[key] = ''
    return
  }
  checks.value[key] = '確認中…'
  try {
    const r = await api.checkPath(p)
    checks.value[key] = !r.exists ? '✕ 見つかりません' : r.kind === 'dir' ? `✓ 写真 ${r.images} 枚` : '✓ ファイルがあります'
  } catch (e) {
    checks.value[key] = '✕ ' + (e as Error).message
  }
}

async function save(): Promise<boolean> {
  if (!cfg.value) return false
  cfg.value.photoDirs = cfg.value.photoDirs.map((d) => d.trim()).filter(Boolean)
  const emails = (t: string) =>
    t
      .split(/[\s,、]+/)
      .map((e) => e.trim().toLowerCase())
      .filter((e) => e.includes('@'))
  cfg.value.adminEmails = emails(adminText.value)
  cfg.value.blockedEmails = emails(blockedText.value)
  cfg.value.allowedOrigins = cfg.value.allowedOrigins.map((o) => o.trim().replace(/\/$/, '')).filter(Boolean)
  if (!cfg.value.photoDirs.length) cfg.value.photoDirs.push('')
  try {
    const r = await api.saveConfig(cfg.value)
    problems.value = r.problems
    warnings.value = r.warnings
    setupCode.value = ''
    dirty.value = false
    toast(r.problems.length ? '保存しました（未入力の項目があります）' : '保存しました')
    return r.problems.length === 0
  } catch (e) {
    toast('保存できませんでした: ' + (e as Error).message)
    return false
  }
}

async function startBuild() {
  if (!(await save())) return
  try {
    await api.build()
    poll()
  } catch (e) {
    toast((e as Error).message)
  }
}

async function poll() {
  build.value = await api.buildStatus()
  if (build.value.running) timer = window.setTimeout(poll, 700)
  else if (build.value.report) {
    toast('構築が完了しました')
    emit('built')
  }
}

async function publish() {
  publishing.value = true
  publishLog.value = ['GitHub に送信しています…']
  try {
    const r = await api.publish(commitMsg.value)
    publishLog.value = r.log
    toast('公開しました')
  } catch (e) {
    publishLog.value = [(e as Error).message]
  } finally {
    publishing.value = false
  }
}

async function showSetup() {
  if (dirty.value) await save()
  setupCode.value = (await api.appsScriptSetup()).code
}
async function copySetup() {
  try {
    await navigator.clipboard.writeText(setupCode.value)
    toast('コピーしました。Apps Script エディタに貼り付けて setupProperties を実行してください')
  } catch {
    toast('コピーできませんでした。枠内を選択してコピーしてください')
  }
}
function genToken() {
  const a = crypto.getRandomValues(new Uint8Array(24))
  cfg.value!.sheetToken = btoa(String.fromCharCode(...a)).replace(/[+/=]/g, '')
  touch()
}

/** 公開 API（トンネルの先の NAS）に外から届くか */
async function checkApi() {
  const u = cfg.value?.publicApiUrl.trim().replace(/\/$/, '')
  if (!u) return
  apiCheck.value = '確認中…'
  try {
    const r = await fetch(`${u}/pub/health`, { cache: 'no-store' })
    const j = await r.json()
    apiCheck.value = j.ok ? `✓ 届きました（${j.ready ? '写真あり' : '写真はまだ構築されていません'}）` : '✕ 応答が正しくありません'
  } catch {
    apiCheck.value = '✕ 届きません（トンネルが動いているか、URL を確認してください）'
  }
}

async function mirrorNow() {
  mirrorMsg.value = '送信中…'
  try {
    const r = await api.mirror()
    mirrorMsg.value = `✓ ${r.sent} 件を送りました`
  } catch (e) {
    mirrorMsg.value = '✕ ' + (e as Error).message
  }
}

const jsOrigins = computed(() => [...(pagesOrigin.value ? [pagesOrigin.value] : []), 'http://localhost:8787'])
const pct = computed(() => (build.value && build.value.total ? Math.round((build.value.done / build.value.total) * 100) : 0))
const pagesOrigin = computed(() => pagesUrl.value.replace(/\/[^/]*\/$/, ''))
const pagesUrl = computed(() => {
  const r = props.local?.git.remote ?? ''
  const mm = r.match(/github\.com[:/]([^/]+)\/([^/.]+)/)
  return mm ? `https://${mm[1].toLowerCase()}.github.io/${mm[2]}/` : ''
})
</script>

<template>
  <section class="settings">
    <template v-if="!local">
      <h1 class="serif">設定は PC の管理ツールで行います</h1>
      <p class="soft">
        写真の登録元（NAS）や名簿、パスワードは、NAS に届く PC で管理ツールを起動して設定します。<br />
        <code>写真</code> フォルダーで <code>cargo run --release</code>（または <code>omuomu-gallery.exe</code>）を実行すると、
        ブラウザで管理画面が開きます。
      </p>
    </template>

    <template v-else-if="cfg">
      <div class="head">
        <div>
          <h1 class="serif">設定</h1>
          <p class="soft">管理ツールの画面です（{{ local.root }}）</p>
        </div>
        <button class="btn" :disabled="!dirty" @click="save">保存</button>
      </div>

      <div v-if="problems.length" class="problems">
        <div v-for="p in problems" :key="p">・{{ p }}</div>
      </div>
      <div v-if="warnings.length" class="warnings">
        <div class="wt">公開前に設定が必要です（この PC での確認はこのままでもできます）</div>
        <div v-for="p in warnings" :key="p">・{{ p }}</div>
      </div>

      <fieldset>
        <legend>写真の登録元（NAS）</legend>
        <p class="help">NAS の写真フォルダーを指定します。例：<code>\\NAS\photo\2023写真</code>、<code>Z:\写真\2023</code>、NAS 上で動かしているときは <code>/srv/nas/photos/2023写真</code>。サブフォルダーも含めて読み込みます。</p>
        <div v-for="(_, i) in cfg.photoDirs" :key="i" class="pathrow">
          <input v-model="cfg.photoDirs[i]" class="input" placeholder="\\NAS\共有フォルダー\写真" @input="touch" @change="check('dir' + i, cfg.photoDirs[i])" />
          <span class="chk" :class="{ ng: checks['dir' + i]?.startsWith('✕') }">{{ checks['dir' + i] }}</span>
          <button v-if="cfg.photoDirs.length > 1" class="x" aria-label="この行を削除" @click="(cfg.photoDirs.splice(i, 1), touch())">×</button>
        </div>
        <button class="add" @click="(cfg.photoDirs.push(''), touch())">＋ フォルダーを追加</button>

        <label class="f">名簿（Excel / CSV）
          <div class="pathrow">
            <input v-model="cfg.rosterPath" class="input" placeholder="..\写真登録.xlsx" @input="touch" @change="check('roster', cfg.rosterPath)" />
            <span class="chk" :class="{ ng: checks.roster?.startsWith('✕') }">{{ checks.roster }}</span>
          </div>
        </label>
        <p class="help">B列=名前、C列=部署。名前の行に貼られた写真は「登録済み」として取り込みます。</p>
      </fieldset>

      <fieldset>
        <legend>展覧会の表示</legend>
        <div class="two">
          <label class="f">タイトル<input v-model="cfg.title" class="input" @input="touch" /></label>
          <label class="f">サブタイトル<input v-model="cfg.subtitle" class="input" @input="touch" /></label>
          <label class="f">会期<input v-model="cfg.period" class="input" placeholder="2026.12.01 ― 12.24" @input="touch" /></label>
          <label class="f">ご案内<input v-model="cfg.notice" class="input" @input="touch" /></label>
        </div>
      </fieldset>

      <fieldset>
        <legend>Google ログイン</legend>
        <p class="help">
          参加者は Google アカウントでログインします。NAS が Google にログイン証明を確かめ、許可した組織（Google Workspace）の
          アカウントと管理者にだけ、写真を 1 枚ずつ返します。設定を変えると、ログイン中の人にも次の操作からすぐ効きます。
        </p>
        <label class="f">OAuth クライアント ID（Google Cloud →「Google Auth Platform」→「クライアント」）
          <input v-model="cfg.googleClientId" class="input" placeholder="1234567890-xxxx.apps.googleusercontent.com" @input="touch" />
        </label>
        <p class="help">クライアントの「承認済みの JavaScript 生成元」には次を登録します: <code v-for="o in jsOrigins" :key="o" class="origin">{{ o }}</code></p>
        <div class="two">
          <label class="f">ログインを許可する組織のドメイン（複数はカンマ区切り）
            <input v-model="cfg.allowedDomain" class="input" placeholder="example.ac.jp" @input="touch" />
          </label>
          <label class="f">ログインの有効時間（時間）
            <input v-model.number="cfg.sessionHours" class="input" type="number" min="1" max="72" @input="touch" />
          </label>
          <label class="f">管理者のメールアドレス（1 行に 1 つ。組織外でも可）
            <textarea v-model="adminText" class="input area" rows="3" placeholder="you@example.ac.jp" @input="touch"></textarea>
          </label>
          <label class="f">入場を止めるアカウント（1 行に 1 つ。すぐに効きます）
            <textarea v-model="blockedText" class="input area" rows="3" placeholder="someone@example.ac.jp" @input="touch"></textarea>
          </label>
        </div>
      </fieldset>

      <fieldset>
        <legend>写真の配信（NAS の公開 API）</legend>
        <p class="help">
          写真と名簿は NAS の外に出しません。参加者のブラウザは、Cloudflare Tunnel / Tailscale Funnel を通って NAS の公開 API から
          写真を受け取ります（手順は nas/README.md）。
        </p>
        <label class="f">公開 API の URL（トンネルで外から届く https のアドレス）
          <div class="pathrow">
            <input v-model="cfg.publicApiUrl" class="input" placeholder="https://photos.example.com" @input="touch" @change="checkApi" />
            <button class="btn small" @click="checkApi">接続を確認</button>
          </div>
        </label>
        <p class="help">{{ apiCheck }}</p>
        <label class="f">公開 API を呼んでよいサイト（1 行に 1 つ。GitHub Pages のアドレス）
          <textarea
            :value="cfg.allowedOrigins.join('\n')"
            class="input area"
            rows="2"
            :placeholder="pagesOrigin || 'https://ユーザー名.github.io'"
            @input="((cfg.allowedOrigins = ($event.target as HTMLTextAreaElement).value.split('\n')), touch())"
          ></textarea>
        </label>
      </fieldset>

      <fieldset>
        <legend>スプレッドシートへの写し（任意）</legend>
        <p class="help">回答と操作ログは NAS に保存されます。Google スプレッドシートにも写したいときは、Apps Script を用意して URL と合言葉を入れます（NAS から 30 秒ごとに送ります）。</p>
        <label class="f">Apps Script ウェブアプリの URL
          <input v-model="cfg.sheetUrl" class="input" placeholder="https://script.google.com/macros/s/…/exec" @input="touch" />
        </label>
        <label class="f">合言葉（NAS と Apps Script で共有）
          <div class="pathrow"><input v-model="cfg.sheetToken" class="input" @input="touch" /><button class="btn small" @click="genToken">生成</button></div>
        </label>
        <div class="kb mt">
          <button class="btn small" :disabled="!cfg.sheetToken" @click="showSetup">Apps Script に貼る設定を表示</button>
          <button class="btn small" :disabled="!cfg.sheetUrl" @click="mirrorNow">今すぐ送る</button>
          <span class="help">{{ mirrorMsg }}</span>
        </div>
        <div v-if="setupCode" class="setup">
          <pre class="log">{{ setupCode }}</pre>
          <button class="btn small fill" @click="copySetup">コピー</button>
        </div>
      </fieldset>

      <fieldset>
        <legend>出やすさ・判定</legend>
        <div class="three">
          <label class="f">回答がこの件数以上で出にくくする<input v-model.number="cfg.threshold" class="input" type="number" min="1" max="100" @input="touch" /></label>
          <label class="f">出にくさ（通常=1、既定 0.03 ≒ 1/30）<input v-model.number="cfg.lowWeight" class="input" type="number" min="0" max="1" step="0.01" @input="touch" /></label>
          <label class="f">「写っている」とみなす得票率<input v-model.number="cfg.majority" class="input" type="number" min="0.1" max="1" step="0.05" @input="touch" /></label>
        </div>
      </fieldset>

      <fieldset>
        <legend>画質</legend>
        <div class="three">
          <label class="f">公開画像の長辺(px)<input v-model.number="cfg.maxSize" class="input" type="number" min="800" max="4000" step="100" @input="touch" /></label>
          <label class="f">サムネイルの長辺(px)<input v-model.number="cfg.thumbSize" class="input" type="number" min="200" max="1000" step="20" @input="touch" /></label>
          <label class="f">JPEG 品質(1-100)<input v-model.number="cfg.jpegQuality" class="input" type="number" min="40" max="100" @input="touch" /></label>
        </div>
      </fieldset>

      <fieldset class="actions">
        <legend>構築と公開</legend>
        <div class="steps">
          <div class="stepbox">
            <div class="label">STEP 1</div>
            <h3 class="serif">構築</h3>
            <p class="help">写真を読み込み、縮小して位置情報などを消し、NAS の中の保存先に置きます。2 回目以降は増えた写真だけ処理します。</p>
            <button class="btn fill" :disabled="build?.running" @click="startBuild">{{ build?.running ? '構築中…' : '保存して構築' }}</button>
            <div v-if="build && (build.running || build.log.length)" class="progress">
              <div class="pbar"><i :style="{ width: (build.running ? pct : 100) + '%' }"></i></div>
              <div class="phase">{{ build.phase }} {{ build.running && build.total ? `${build.done} / ${build.total}` : '' }}</div>
              <pre class="log">{{ build.log.join('\n') }}</pre>
            </div>
          </div>
          <div class="stepbox">
            <div class="label">STEP 2</div>
            <h3 class="serif">公開</h3>
            <p class="help">
              GitHub に送るのは表示用の文言と接続先（public.json）だけです。写真・名簿・回答は NAS の外に出ません。
              ブランチ: <b>{{ local.git.branch ?? '不明' }}</b>
            </p>
            <input v-model="commitMsg" class="input" aria-label="コミットメッセージ" />
            <button class="btn fill mt" :disabled="publishing || build?.running" @click="publish">{{ publishing ? '送信中…' : 'GitHub に公開' }}</button>
            <pre v-if="publishLog.length" class="log">{{ publishLog.join('\n') }}</pre>
            <p v-if="pagesUrl" class="help">公開先: <a :href="pagesUrl" target="_blank" rel="noopener">{{ pagesUrl }}</a></p>
          </div>
        </div>
        <p class="help">構築後は <a href="#/" target="_blank">この PC で入口を開いて</a> 表示を確かめられます。</p>
      </fieldset>
    </template>
    <p v-else class="soft">読み込み中…</p>
  </section>
</template>

<style scoped>
.settings {
  max-width: 980px;
  margin: 0 auto;
  padding: 32px 32px 80px;
}
h1 {
  font-weight: 500;
  font-size: 24px;
  letter-spacing: 0.12em;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: flex-end;
  gap: 16px;
  position: sticky;
  top: var(--top-h);
  z-index: 5;
  background: var(--paper-2);
  padding: 12px 0;
  border-bottom: 1px solid var(--line);
}
.soft {
  color: var(--ink-soft);
  font-size: 12px;
  line-height: 1.9;
  margin-top: 6px;
}
code {
  font-family: Consolas, 'Cascadia Mono', ui-monospace, monospace;
  background: color-mix(in srgb, var(--line) 50%, transparent);
  padding: 1px 6px;
  font-size: 12px;
}
.warnings {
  margin-top: 12px;
  padding: 12px 16px;
  border: 1px dashed var(--fixed-line);
  background: var(--fixed-bg);
  font-size: 12px;
  line-height: 1.8;
}
.wt {
  font-weight: 500;
  margin-bottom: 4px;
}
.area {
  resize: vertical;
  font-size: 14px;
  line-height: 1.6;
}
.origin {
  margin: 0 4px;
}
.keybox {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 16px;
  flex-wrap: wrap;
  margin-top: 20px;
  padding: 16px;
  border: 1px solid var(--line);
  border-radius: 10px;
  background: var(--paper);
}
.key {
  display: inline-block;
  margin-top: 6px;
  font-size: 13px;
}
.kb {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.setup {
  margin-top: 12px;
}
.setup .btn {
  margin-top: 8px;
}
.problems {
  margin-top: 16px;
  padding: 12px 16px;
  border: 1px solid var(--dot);
  color: var(--dot);
  font-size: 13px;
  line-height: 1.8;
}
fieldset {
  border: none;
  border-top: 1px solid var(--line);
  margin-top: 28px;
  padding-top: 18px;
}
legend {
  font-family: var(--serif);
  font-size: 16px;
  font-weight: 500;
  letter-spacing: 0.1em;
  padding-right: 12px;
}
.help {
  font-size: 12px;
  color: var(--ink-soft);
  line-height: 1.8;
  margin: 6px 0 10px;
}
.help a {
  color: var(--accent);
}
.f {
  display: block;
  font-size: 12px;
  letter-spacing: 0.06em;
  color: var(--ink-soft);
  margin-top: 14px;
}
.f .input {
  color: var(--ink);
}
.pathrow {
  display: flex;
  gap: 12px;
  align-items: flex-end;
  margin-top: 4px;
}
.chk {
  font-size: 12px;
  white-space: nowrap;
  color: var(--accent);
  min-width: 90px;
}
.chk.ng {
  color: var(--dot);
}
.x {
  font-size: 18px;
  color: var(--ink-soft);
  padding: 4px 8px;
}
.add {
  margin-top: 10px;
  font-size: 12px;
  color: var(--accent);
  letter-spacing: 0.1em;
}
.two {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0 28px;
}
.three {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 0 28px;
}
.check {
  display: flex;
  gap: 8px;
  align-items: center;
  font-size: 12px;
  margin-top: 12px;
}
.check input {
  accent-color: var(--accent);
}
.steps {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 20px;
  margin-top: 8px;
}
.stepbox {
  background: var(--paper);
  border: 1px solid var(--line);
  border-radius: 10px;
  padding: 20px;
}
.stepbox h3 {
  font-weight: 500;
  font-size: 18px;
  letter-spacing: 0.1em;
  margin: 4px 0 4px;
}
.mt {
  margin-top: 14px;
}
.progress {
  margin-top: 16px;
}
.pbar {
  height: 3px;
  background: var(--line);
}
.pbar i {
  display: block;
  height: 100%;
  background: var(--accent);
  transition: width 0.4s;
}
.phase {
  font-size: 12px;
  margin-top: 6px;
}
.log {
  margin-top: 10px;
  max-height: 220px;
  overflow: auto;
  font-size: 11px;
  line-height: 1.7;
  white-space: pre-wrap;
  word-break: break-all;
  background: var(--paper-2);
  border: 1px solid var(--line);
  padding: 10px;
  font-family: ui-monospace, 'Cascadia Mono', monospace;
}
@media (max-width: 760px) {
  .settings {
    padding: 16px 16px 60px;
  }
  .two,
  .three,
  .steps {
    grid-template-columns: 1fr;
  }
}
</style>
