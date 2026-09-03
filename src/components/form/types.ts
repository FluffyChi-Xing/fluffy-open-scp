export type FormFieldType = 'text' | 'textarea' | 'select' | 'checkbox'

export interface FormOption {
  label: string
  value: string
}

export interface FormColumn<Model extends object> {
  field: Extract<keyof Model, string>
  labelKey: string
  type: FormFieldType
  placeholderKey?: string
  helpKey?: string
  required?: boolean
  pattern?: RegExp
  validate?: (value: Model[keyof Model], values: Model) => string | undefined
  defaultValue?: Model[keyof Model]
  options?: FormOption[]
  visible?: (values: Model) => boolean
  disabled?: (values: Model) => boolean
  span?: 1 | 2
}
