<script setup lang="ts">
import { Switch } from "@/components/ui/switch";
import { useI18n } from "vue-i18n";
const { t } = useI18n();
/**
 * 步骤 ②「菜单提示」表单：hover 弹窗的全部内容 ——
 * 描述文案、rollover 大图（454×263）、造价/预算、解锁提示、锁定态。
 */
const model = defineModel<{
  desc: string;
  cost: string;
  upkeep: string;
  unlock: string;
  locked: boolean;
  marquee: string | null;
}>({ required: true });

const emit = defineEmits<{ (e: "crop"): void }>();
</script>

<template>
  <div class="form">
    <label class="field">
      <span>{{ t('studio.workbench.tipDescription') }}</span>
      <textarea v-model="model.desc" rows="3" />
    </label>
    <div class="field">
      <span>{{ t('studio.workbench.tipImage') }}</span>
      <button type="button" class="img-btn" @click="emit('crop')">
        <img v-if="model.marquee" :src="model.marquee" alt="" />
        <span v-else class="img-btn-hint">{{ t('studio.workbench.uploadTip') }}</span>
      </button>
    </div>
    <div class="field-row">
      <label class="field">
        <span>{{ t('studio.workbench.cost') }}</span>
        <input v-model="model.cost" type="text" placeholder="27,500" />
      </label>
      <label class="field">
        <span>{{ t('studio.workbench.upkeep') }}</span>
        <input v-model="model.upkeep" type="text" placeholder="-856" />
      </label>
    </div>
    <label class="field">
      <span>{{ t('studio.workbench.unlock') }}</span>
      <input v-model="model.unlock" type="text" />
    </label>
    <label class="field-check">
      <Switch v-model="model.locked" :aria-label="t('studio.workbench.locked')" />
      <span>{{ t('studio.workbench.locked') }}</span>
    </label>
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
.field textarea {
  height: auto;
  resize: vertical;
}
.field-check {
  align-items: center;
  display: flex;
  flex-direction: row;
  gap: 6px;
}
.field-check span {
  color: var(--foreground);
  font-size: 12px;
}
.img-btn {
  align-items: center;
  background: var(--surface);
  border: 1px dashed var(--border-strong);
  border-radius: var(--radius-sm);
  color: var(--muted-foreground);
  cursor: pointer;
  display: inline-flex;
  height: 96px;
  justify-content: center;
  overflow: hidden;
  width: 100%;
}
.img-btn:hover {
  border-color: var(--primary, #0b78fe);
}
.img-btn img {
  height: 100%;
  object-fit: cover;
  width: 100%;
}
.img-btn-hint {
  font-size: 11px;
  padding: 0 6px;
  text-align: center;
}
</style>
