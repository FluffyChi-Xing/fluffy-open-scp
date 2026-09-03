<script setup lang="ts">
import { computed, shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute, useRouter } from 'vue-router'
import FButton from '@/components/ui/FButton.vue'
import FFormItem from '@/components/ui/FFormItem.vue'
import FInput from '@/components/ui/FInput.vue'
import { useToast } from '@/composables/useToast'

const email = shallowRef('demo@fluffy.dev')
const password = shallowRef('')
const router = useRouter()
const route = useRoute()
const { t } = useI18n()
const { success } = useToast()
const redirect = computed(() => typeof route.query.redirect === 'string' && route.query.redirect.startsWith('/') && !route.query.redirect.startsWith('//') ? route.query.redirect : '/')
function login() { success(t('login.success')); router.push(redirect.value) }
</script>

<template><main class="login-page"><section class="brand-panel"><div class="brand-copy"><p>{{ $t('login.eyebrow') }}</p><h1>{{ $t('login.slogan') }}</h1><span>{{ $t('login.description') }}</span></div></section><section class="login-panel"><form class="login-form" @submit.prevent="login"><div><p class="eyebrow">{{ $t('login.eyebrow') }}</p><h2>{{ $t('login.welcome') }}</h2><p>{{ $t('login.detail') }}</p></div><FFormItem id="login-email" :label="$t('login.email')"><template #default="field"><FInput :id="field.id" v-model="email" type="email" :placeholder="$t('form.emailPlaceholder')" /></template></FFormItem><FFormItem id="login-password" :label="$t('login.password')"><template #default="field"><FInput :id="field.id" v-model="password" type="password" :placeholder="$t('login.passwordPlaceholder')" /></template></FFormItem><FButton type="submit">{{ $t('login.submit') }}</FButton></form></section></main></template>

<style scoped>
.login-page{background:var(--background);display:grid;grid-template-columns:minmax(360px,1fr) minmax(420px,1fr);min-height:100vh}.brand-panel{background:linear-gradient(135deg,var(--brand),color-mix(in srgb,var(--brand) 58%,#0f172a));color:var(--primary-foreground);display:flex;padding:clamp(32px,8vw,112px)}.brand-copy{align-self:flex-end;display:grid;gap:18px;max-width:500px}.brand-copy p,.eyebrow{font-size:11px;font-weight:750;letter-spacing:.1em;margin:0;text-transform:uppercase}.brand-copy h1{font-size:clamp(2.6rem,5vw,5rem);letter-spacing:-.065em;line-height:.96;margin:0}.brand-copy span{font-size:15px;line-height:1.65;max-width:390px;opacity:.84}.login-panel{align-items:center;display:flex;justify-content:center;padding:32px}.login-form{display:grid;gap:20px;max-width:370px;width:100%}.eyebrow{color:var(--primary);margin-bottom:10px}.login-form h2{font-size:28px;letter-spacing:-.045em;margin:0}.login-form>div>p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;line-height:1.55;margin:9px 0 0}@media(max-width:760px){.login-page{grid-template-columns:1fr}.brand-panel{display:none}.login-panel{padding:24px}}
</style>
