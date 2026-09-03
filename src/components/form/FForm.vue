<script setup lang="ts">
import { computed } from 'vue'
import FButton from '@/components/ui/FButton.vue'
import FCheckbox from '@/components/ui/FCheckbox.vue'
import FFormItem from '@/components/ui/FFormItem.vue'
import FInput from '@/components/ui/FInput.vue'
import FSelect from '@/components/ui/FSelect.vue'
import FTextarea from '@/components/ui/FTextarea.vue'
import type { FormColumn } from '@/components/form/types'
import { useForm } from '@/composables/useForm'

type FormModel = Record<string, string | boolean>
interface Props { columns: FormColumn<FormModel>[]; loading?: boolean; submitKey?: string }
interface Emits { submit: [values: Readonly<FormModel>]; reset: [] }
const props = withDefaults(defineProps<Props>(), { submitKey: 'form.submit' })
const emit = defineEmits<Emits>()
const model = defineModel<FormModel>({ required: true })
const { values, errors, validateField, reset, submit } = useForm(model.value, props.columns)
const visibleColumns = computed(() => props.columns.filter((column) => column.visible?.(values) ?? true))

async function onSubmit() {
  await submit(async (nextValues) => {
    model.value = { ...nextValues }
    emit('submit', nextValues)
  })()
}

function onReset() {
  reset()
  model.value = { ...values }
  emit('reset')
}
</script>

<template>
  <form class="f-form" @submit.prevent="onSubmit" @reset.prevent="onReset">
    <div class="f-form-fields">
      <FFormItem v-for="column in visibleColumns" :key="column.field" :id="`field-${column.field}`" :label="$t(column.labelKey)" :required="column.required" :help="column.helpKey ? $t(column.helpKey) : undefined" :error="errors[column.field] ? $t(errors[column.field] as string) : undefined" :class="{ wide: column.span === 2 }">
        <template #default="field">
          <slot :name="`field-${column.field}`" :column="column" :values="values" :error="errors[column.field]">
            <FInput v-if="column.type === 'text'" :id="field.id" v-model="values[column.field] as string" :placeholder="column.placeholderKey ? $t(column.placeholderKey) : undefined" :disabled="column.disabled?.(values)" :invalid="Boolean(errors[column.field])" :aria-describedby="field.describedBy" @blur="validateField(column.field)" />
            <FTextarea v-else-if="column.type === 'textarea'" :id="field.id" v-model="values[column.field] as string" :placeholder="column.placeholderKey ? $t(column.placeholderKey) : undefined" :disabled="column.disabled?.(values)" :invalid="Boolean(errors[column.field])" :aria-describedby="field.describedBy" @blur="validateField(column.field)" />
            <FSelect v-else-if="column.type === 'select'" :id="field.id" v-model="values[column.field] as string" :options="column.options ?? []" :disabled="column.disabled?.(values)" :invalid="Boolean(errors[column.field])" :aria-describedby="field.describedBy" @change="validateField(column.field)" />
            <FCheckbox v-else :id="field.id" v-model="values[column.field] as boolean" :disabled="column.disabled?.(values)" @change="validateField(column.field)" />
          </slot>
        </template>
      </FFormItem>
    </div>
    <footer class="f-form-actions"><FButton variant="secondary" type="reset">{{ $t('form.reset') }}</FButton><FButton type="submit" :loading="props.loading">{{ $t(props.submitKey) }}</FButton></footer>
  </form>
</template>

<style scoped>
.f-form{display:grid;gap:20px}.f-form-fields{display:grid;gap:17px;grid-template-columns:repeat(2,minmax(0,1fr))}.wide{grid-column:span 2}.f-form-actions{display:flex;gap:10px;justify-content:flex-end}@media(max-width:620px){.f-form-fields{grid-template-columns:1fr}.wide{grid-column:span 1}.f-form-actions{justify-content:stretch}.f-form-actions>*{flex:1}}
</style>
