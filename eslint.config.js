import eslint from '@eslint/js'
import prettier from 'eslint-config-prettier'
import globals from 'globals'
import tseslint from 'typescript-eslint'
import vue from 'eslint-plugin-vue'

// 文件长度纪律(用户指令 2026-10-27):新增 .ts 单文件 ≤1000 行,语义连贯,
// 尽量纯函数。存量超限文件在此挂账(存量已冻结的 property-editor 目录除外),
// 重构迁移后逐个移除名单——名单只允许缩短,不允许加长。
const legacyOversizeFiles = [
  'src/locales/index.ts',
  'src/pages/packages/components/property-editor/refinedRender.ts',
  'src/pages/studio/components/asset/useWorkbenchAsset.ts',
  'src/api/tauri.ts',
  'src/stores/uiWorkbench.ts',
  'src/api/data-source.ts'
]

export default tseslint.config(
  {
    ignores: ['.claude/**', 'dist/**', 'target/**', 'node_modules/**', 'src-tauri/gen/**', 'tools/**', 'website/**', 'output/**', 'tmp/**']
  },
  eslint.configs.recommended,
  ...tseslint.configs.recommended,
  ...vue.configs['flat/recommended'],
  {
    files: ['**/*.{js,mjs,cjs,ts,vue}'],
    languageOptions: {
      globals: {
        ...globals.browser,
        ...globals.node
      }
    },
    rules: {
      'vue/multi-word-component-names': 'off',
      'vue/no-v-html': 'off',
      'vue/no-restricted-html-elements': ['error', { element: 'select', message: 'Use FDropdown (or the FSelect wrapper backed by FDropdown), never a native select.' }],
      'vue/require-default-prop': 'off',
      'vue/attributes-order': 'off',
      'vue/one-component-per-file': 'off'
    }
  },
  {
    files: ['**/*.ts'],
    ignores: legacyOversizeFiles,
    rules: {
      'max-lines': ['error', { max: 1000, skipBlankLines: false, skipComments: false }]
    }
  },
  {
    files: ['**/*.vue'],
    languageOptions: {
      parserOptions: {
        parser: tseslint.parser
      }
    }
  },
  prettier
)
