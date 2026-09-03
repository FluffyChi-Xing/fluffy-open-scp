<script setup lang="ts">
import { shallowRef } from 'vue'
import { useI18n } from 'vue-i18n'
import FForm from '@/components/form/FForm.vue'
import type { FormColumn } from '@/components/form/types'
import FButton from '@/components/ui/FButton.vue'
import FCheckbox from '@/components/ui/FCheckbox.vue'
import FFormItem from '@/components/ui/FFormItem.vue'
import FInput from '@/components/ui/FInput.vue'
import FPanel from '@/components/ui/FPanel.vue'
import FTextarea from '@/components/ui/FTextarea.vue'
import { useToast } from '@/composables/useToast'

type ProfileForm = Record<'name' | 'email' | 'role' | 'bio', string> & Record<'subscribe', boolean>
const { t } = useI18n()
const { success } = useToast()
const manualName = shallowRef('Fluffy')
const manualEmail = shallowRef('hello@fluffy.dev')
const autoProfile = shallowRef<ProfileForm>({ name: '', email: '', role: 'developer', bio: '', subscribe: true })
const columns: FormColumn<Record<string, string | boolean>>[] = [
  { field: 'name', labelKey: 'form.name', placeholderKey: 'form.namePlaceholder', type: 'text', required: true },
  { field: 'email', labelKey: 'form.email', placeholderKey: 'form.emailPlaceholder', type: 'text', required: true, pattern: /^[^\s@]+@[^\s@]+\.[^\s@]+$/ },
  { field: 'role', labelKey: 'form.role', type: 'select', options: [{ value: 'developer', label: 'Developer' }, { value: 'designer', label: 'Designer' }, { value: 'operator', label: 'Operator' }] },
  { field: 'bio', labelKey: 'form.bio', placeholderKey: 'form.bioPlaceholder', type: 'textarea', span: 2 },
  { field: 'subscribe', labelKey: 'form.subscribe', type: 'checkbox', span: 2 }
]
function submitManual() { success(t('form.saved')) }
function submitAuto() { success(t('form.saved')) }
</script>

<template><section class="page"><header><p class="eyebrow">{{ $t('showcase.eyebrow') }}</p><h1>{{ $t('showcase.formsTitle') }}</h1><p>{{ $t('showcase.formsDescription') }}</p></header><div class="form-grid"><FPanel><h2>{{ $t('showcase.manualForm') }}</h2><form class="manual-form" @submit.prevent="submitManual"><FFormItem id="manual-name" :label="$t('form.name')"><template #default="field"><FInput :id="field.id" v-model="manualName" :placeholder="$t('form.namePlaceholder')" /></template></FFormItem><FFormItem id="manual-email" :label="$t('form.email')"><template #default="field"><FInput :id="field.id" v-model="manualEmail" type="email" :placeholder="$t('form.emailPlaceholder')" /></template></FFormItem><FFormItem id="manual-bio" :label="$t('form.bio')"><template #default="field"><FTextarea :id="field.id" :placeholder="$t('form.bioPlaceholder')" /></template></FFormItem><FButton type="submit">{{ $t('form.submit') }}</FButton></form></FPanel><FPanel><h2>{{ $t('showcase.autoForm') }}</h2><FForm v-model="autoProfile" :columns="columns" @submit="submitAuto"><template #field-subscribe="{ column }"><label class="checkbox-line"><FCheckbox v-model="autoProfile.subscribe" />{{ $t(column.labelKey) }}</label></template></FForm></FPanel></div></section></template>

<style scoped>
.page{display:grid;gap:24px}.eyebrow{color:var(--primary);font-size:11px;font-weight:750;letter-spacing:.08em;margin:0 0 10px;text-transform:uppercase}.page h1{font-size:clamp(1.8rem,3vw,2.5rem);letter-spacing:-.045em;margin:0}.page>header>p:not(.eyebrow){color:var(--muted-foreground);font-size:14px;margin:10px 0 0}.form-grid{display:grid;gap:18px;grid-template-columns:repeat(2,minmax(0,1fr))}.form-grid h2{font-size:15px;margin:0 0 18px}.manual-form{display:grid;gap:17px}.checkbox-line{align-items:center;color:var(--foreground);display:flex;font-size:13px;font-weight:650;gap:9px;min-height:38px}@media(max-width:800px){.form-grid{grid-template-columns:1fr}}
</style>
