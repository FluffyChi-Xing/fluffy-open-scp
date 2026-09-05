<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import FButton from "@/components/ui/FButton.vue";
import FCheckbox from "@/components/ui/FCheckbox.vue";
import FCode from "@/components/ui/FCode.vue";
import FEmpty from "@/components/extensions/FEmpty.vue";
import FFormItem from "@/components/ui/FFormItem.vue";
import FInput from "@/components/ui/FInput.vue";
import FMarkdown from "@/components/markdown/FMarkdown.vue";
import FPanel from "@/components/ui/FPanel.vue";
import FResult from "@/components/ui/FResult.vue";
import FSelect from "@/components/ui/FSelect.vue";
import FSkeleton from "@/components/ui/FSkeleton.vue";
import FSpinner from "@/components/ui/FSpinner.vue";
import FTextarea from "@/components/ui/FTextarea.vue";
import { useToast } from "@/composables/useToast";

const { success, info, warning, error } = useToast();
const { t } = useI18n();

const text = ref("");
const email = ref("");
const password = ref("");
const textarea = ref("");
const select = ref("option-a");
const checked = ref(true);
const selectOptions = computed(() => [
  { label: t("components.optionA"), value: "option-a" },
  { label: t("components.optionB"), value: "option-b" },
  { label: t("components.optionC"), value: "option-c" },
]);
const buttonLoading = ref(false);
function simulateLoading() {
  buttonLoading.value = true;
  window.setTimeout(() => {
    buttonLoading.value = false;
  }, 1200);
}
const sampleCode = [
  "const greeting = '你好，Fluffy'",
  "export function welcome(name: string): string {",
  "  return `${greeting}，${name}`",
  "}",
].join("\n");
const markdownSource = [
  "# Fluffy Design Pro",
  "",
  "一个为管理控制台打造的轻量前端基础层。",
  "",
  "## 特性",
  "",
  "- **轻量**：不引入完整 UI 框架",
  "- **可复用**：`f-` 前缀组件与组合式逻辑",
  "- **可测试**：内置 Vitest 测试基础",
  "",
  "```ts",
  'const name = "Fluffy"',
  "console.log(welcome(name))",
  "```",
  "",
  "> 代码块由 FCode 渲染，支持语法高亮、折叠与复制。",
].join("\n");
</script>

<template>
  <section class="page">
    <header>
      <p class="eyebrow">{{ $t("showcase.eyebrow") }}</p>
      <h1>{{ $t("showcase.componentsTitle") }}</h1>
      <p>{{ $t("showcase.componentsDescription") }}</p>
    </header>
    <FPanel
      ><h2>{{ $t("components.buttons") }}</h2>
      <div class="demo-row">
        <FButton>{{ $t("components.primary") }}</FButton
        ><FButton variant="secondary">{{ $t("components.secondary") }}</FButton
        ><FButton variant="ghost">{{ $t("components.ghost") }}</FButton
        ><FButton variant="danger">{{ $t("components.danger") }}</FButton
        ><FButton :loading="buttonLoading" @click="simulateLoading">{{
          $t("components.loadingDemo")
        }}</FButton
        ><FButton variant="secondary" disabled>{{
          $t("components.disabled")
        }}</FButton>
      </div></FPanel
    ><FPanel
      ><h2>{{ $t("components.inputs") }}</h2>
      <div class="demo-grid">
        <FFormItem id="demo-text" :label="$t('components.text')"
          ><template #default="field"
            ><FInput
              :id="field.id"
              v-model="text"
              :placeholder="$t('components.textPlaceholder')"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem id="demo-email" :label="$t('components.email')"
          ><template #default="field"
            ><FInput
              :id="field.id"
              v-model="email"
              type="email"
              :placeholder="$t('form.emailPlaceholder')"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem id="demo-password" :label="$t('components.password')"
          ><template #default="field"
            ><FInput
              :id="field.id"
              v-model="password"
              type="password"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem id="demo-select" :label="$t('components.select')"
          ><template #default="field"
            ><FSelect
              :id="field.id"
              v-model="select"
              :options="selectOptions"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem id="demo-textarea" :label="$t('components.textarea')"
          ><template #default="field"
            ><FTextarea
              :id="field.id"
              v-model="textarea"
              :rows="3"
              :placeholder="$t('components.textareaPlaceholder')"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem id="demo-checkbox" :label="$t('components.checkbox')"
          ><template #default="field"
            ><FCheckbox
              :id="field.id"
              v-model="checked"
              :aria-describedby="field.describedBy" /></template
        ></FFormItem></div></FPanel
    ><FPanel
      ><h2>{{ $t("components.formField") }}</h2>
      <div class="demo-grid">
        <FFormItem
          id="demo-required"
          :label="$t('components.requiredLabel')"
          required
          ><template #default="field"
            ><FInput
              :id="field.id"
              v-model="text"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem
          id="demo-help"
          :label="$t('components.helpLabel')"
          :help="$t('components.helpHint')"
          ><template #default="field"
            ><FInput
              :id="field.id"
              v-model="text"
              :aria-describedby="field.describedBy" /></template></FFormItem
        ><FFormItem
          id="demo-error"
          :label="$t('components.errorLabel')"
          :error="$t('components.errorHint')"
          ><template #default="field"
            ><FInput
              :id="field.id"
              v-model="text"
              invalid
              :aria-describedby="field.describedBy" /></template
        ></FFormItem></div></FPanel
    ><FPanel
      ><h2>{{ $t("components.loading") }}</h2>
      <div class="demo-row loading-demo">
        <span class="spinner-demo"
          ><FSpinner size="sm" :label="$t('components.spinnerSm')" />{{
            $t("components.spinnerSm")
          }}</span
        ><span class="spinner-demo"
          ><FSpinner :label="$t('components.spinnerMd')" />{{
            $t("components.spinnerMd")
          }}</span
        >
        <div class="skeleton-demo">
          <FSkeleton width="46%" height="12px" /><FSkeleton
            height="16px"
          /><FSkeleton width="72%" height="16px" /><FSkeleton
            width="112px"
            height="34px"
            rounded
          />
        </div></div></FPanel
    ><FPanel
      ><h2>{{ $t("components.code") }}</h2>
      <FCode
        :code="sampleCode"
        lang="ts"
        :copy-label="$t('code.copy')"
        :copied-label="$t('code.copied')"
        :collapse-label="$t('code.collapse')"
        :expand-label="$t('code.expand')" /></FPanel
    ><FPanel
      ><h2>{{ $t("components.markdown") }}</h2>
      <FMarkdown :source="markdownSource" /></FPanel
    ><FPanel
      ><h2>{{ $t("components.results") }}</h2>
      <div class="results-grid">
        <FResult
          tone="success"
          :title="$t('components.resultSuccess')"
          :description="$t('components.resultSuccessDesc')"
        /><FResult
          tone="info"
          :title="$t('components.resultInfo')"
          :description="$t('components.resultInfoDesc')"
        /><FResult
          tone="warning"
          :title="$t('components.resultWarning')"
          :description="$t('components.resultWarningDesc')"
        /><FResult
          tone="error"
          :title="$t('components.resultError')"
          :description="$t('components.resultErrorDesc')"
        /></div></FPanel
    ><FPanel
      ><h2>{{ $t("components.empty") }}</h2>
      <div class="empty-grid">
        <FEmpty
          :title="$t('components.emptyTitle')"
          :desc="$t('components.emptyDesc')"
          ><FButton>{{ $t("components.emptyAction") }}</FButton></FEmpty
        ><FEmpty
          variant="compact"
          status="warning"
          icon-name="Search"
          :title="$t('components.emptyFilteredTitle')"
          :desc="$t('components.emptyFilteredDesc')"
        /></div></FPanel
    ><FPanel
      ><h2>{{ $t("components.toast") }}</h2>
      <div class="demo-row">
        <FButton @click="success($t('toast.success'))">{{
          $t("components.toastSuccess")
        }}</FButton
        ><FButton variant="secondary" @click="info($t('toast.info'))">{{
          $t("components.toastInfo")
        }}</FButton
        ><FButton variant="secondary" @click="warning($t('toast.warning'))">{{
          $t("components.toastWarning")
        }}</FButton
        ><FButton variant="danger" @click="error($t('toast.error'))">{{
          $t("components.toastError")
        }}</FButton>
      </div></FPanel
    >
  </section>
</template>

<style scoped>
.page {
  display: grid;
  gap: 18px;
}
.eyebrow {
  color: var(--primary);
  font-size: 11px;
  font-weight: 750;
  letter-spacing: 0.08em;
  margin: 0 0 10px;
  text-transform: uppercase;
}
h1 {
  font-size: clamp(1.8rem, 3vw, 2.5rem);
  letter-spacing: -0.045em;
  margin: 0;
}
header p:not(.eyebrow) {
  color: var(--muted-foreground);
  font-size: 14px;
  margin: 10px 0 0;
}
.page h2 {
  font-size: 14px;
  margin: 0 0 18px;
}
.demo-row {
  align-items: center;
  display: flex;
  flex-wrap: wrap;
  gap: 10px;
}
.demo-grid {
  display: grid;
  gap: 18px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.loading-demo {
  gap: 26px;
}
.empty-grid {
  display: grid;
  gap: 20px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.empty-grid .f-empty {
  justify-self: center;
  width: 100%;
}
.spinner-demo {
  align-items: center;
  color: var(--muted-foreground);
  display: flex;
  font-size: 13px;
  gap: 8px;
}
.skeleton-demo {
  display: grid;
  gap: 11px;
  min-width: 220px;
}
.results-grid {
  display: grid;
  gap: 24px;
  grid-template-columns: repeat(2, minmax(0, 1fr));
}
.results-grid .f-result {
  justify-self: center;
  width: 100%;
}
.upload-demo-list {
  display: grid;
  gap: 4px;
  list-style: none;
  margin: 0;
  padding: 0;
}
.upload-demo-list li {
  background: var(--surface-hover);
  border-radius: var(--radius-sm);
  padding: 10px;
}
.upload-demo-empty {
  color: var(--muted-foreground);
  font-size: 13px;
  padding: 12px 0;
}
@media (max-width: 720px) {
  .demo-grid,
  .results-grid {
    grid-template-columns: 1fr;
  }
}
</style>
