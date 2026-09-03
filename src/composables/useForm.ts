import { computed, reactive, readonly } from 'vue'
import type { FormColumn } from '@/components/form/types'

export type FormErrors = Record<string, string | undefined>

export function useForm<Model extends object>(
  initialValues: Model,
  columns: FormColumn<Model>[]
) {
  const values = reactive({ ...initialValues }) as Model
  const errors = reactive<FormErrors>({})
  const touched = reactive<Record<string, boolean>>({})
  const isValid = computed(() => !Object.values(errors).some(Boolean))

  function validateField(field: Extract<keyof Model, string>) {
    const column = columns.find((item) => item.field === field)
    if (!column) return undefined
    const value = values[field]
    let error: string | undefined

    if (column.required && (value === '' || value === false || value === undefined || value === null)) error = 'form.required'
    else if (column.pattern && typeof value === 'string' && !column.pattern.test(value)) error = 'form.pattern'
    else error = column.validate?.(value, values)

    errors[field] = error
    return error
  }

  function validate() {
    for (const column of columns) validateField(column.field)
    return isValid.value
  }

  function reset() {
    Object.assign(values, initialValues)
    for (const key of Object.keys(errors)) delete errors[key]
    for (const key of Object.keys(touched)) delete touched[key]
  }

  function submit(handler: (values: Model) => void | Promise<void>) {
    return async () => {
      for (const column of columns) touched[column.field] = true
      if (!validate()) return false
      await handler({ ...values } as Model)
      return true
    }
  }

  return { values, errors: readonly(errors), touched: readonly(touched), isValid, validateField, validate, reset, submit }
}
