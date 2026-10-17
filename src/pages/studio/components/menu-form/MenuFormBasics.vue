<script setup lang="ts">
/**
 * 步骤 ①「菜单条目」表单：显示名称、排序、FIcon 覆盖、槽位图标（128×128）。
 * 裁切图片只负责产出 dataURL 并回写 model，弹窗交互由宿主（Sheet）承接。
 */
import { useI18n } from "vue-i18n";
const { t } = useI18n();
const model = defineModel<{
  label: string;
  pos: number;
  icon: string;
  preview: string | null;
}>({ required: true });

const emit = defineEmits<{ (e: "crop"): void }>();
</script>

<template>
  <div class="form">
    <label class="field">
      <span>{{ t('studio.workbench.fieldLabel') }}</span>
      <input v-model="model.label" type="text" />
    </label>
    <div class="field-row">
      <label class="field">
        <span>{{ t('studio.workbench.fieldPos') }}</span>
        <input v-model.number="model.pos" type="number" />
      </label>
      <label class="field">
        <span>{{ t('studio.workbench.fieldIcon') }}</span>
        <input v-model="model.icon" type="text" placeholder="Box" />
      </label>
    </div>
    <p class="field-hint">{{ t('studio.workbench.iconHint') }}</p>
    <div class="field">
      <span>{{ t('studio.workbench.slotIcon') }}</span>
      <button type="button" class="img-btn" @click="emit('crop')">
        <img v-if="model.preview" :src="model.preview" alt="" />
        <span v-else class="img-btn-hint">{{ t('studio.workbench.uploadSlot') }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.form {
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.field {
  display: flex;
  flex-direction: column;
  font-size: 12px;
  gap: 4px;
  min-width: 0;
}
.field > span {
  color: var(--muted-foreground);
}
.field-row {
  display: grid;
  gap: 12px;
  grid-template-columns: 1fr 1fr;
}
.field input,
.field textarea {
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius-sm);
  color: var(--foreground);
  font: inherit;
  font-size: 13px;
  height: 32px;
  min-width: 0;
  padding: 5px 9px;
  width: 100%;
}
.field-hint {
  color: var(--subtle-foreground);
  font-size: 11px;
  margin: 0;
}
.img-btn {
  align-items: center;
  background: var(--surface);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 84px;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.img-btn:hover {
  border-color: var(--primary, #0b78fe);
}
.img-btn img {
  height: 100%;
  object-fit: contain;
  width: 100%;
}
.img-btn-hint {
  font-size: 11px;
  padding: 0 6px;
  text-align: center;
}
</style>
