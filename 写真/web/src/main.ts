import { createApp } from 'vue'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import EntranceView from './views/EntranceView.vue'
import CardView from './views/CardView.vue'
import AdminView from './views/AdminView.vue'
import { restoreSession, store } from './lib/store'
import './styles/base.css'

const router = createRouter({
  // GitHub Pages ではサーバー側のルーティングが無いのでハッシュ方式
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: EntranceView },
    { path: '/cards', component: CardView, meta: { needsView: true } },
    { path: '/admin', component: AdminView },
    { path: '/:p(.*)*', redirect: '/' },
  ],
})

router.beforeEach(async (to) => {
  await restoreSession()
  if (to.meta.needsView && !store.manifest) return '/'
  if (to.path === '/' && store.manifest && !to.query.lock) return '/cards'
})

createApp(App).use(router).mount('#app')
