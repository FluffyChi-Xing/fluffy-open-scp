<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Resizable,
  ResizableHandle,
  ResizablePanel,
} from "@/components/ui/resizable";

const { t } = useI18n();
const columnHandleMargins = { coarse: 0, fine: 0 };
</script>

<template>
  <Resizable direction="horizontal" class="maw-split">
    <ResizablePanel id="maw-tree" :default-size="17" :min-size="0">
      <slot name="files" />
    </ResizablePanel>
    <ResizableHandle
      orientation="horizontal"
      :hit-area-margins="columnHandleMargins"
      :label="t('studio.assetWorkbench.resizeTree')"
    />
    <ResizablePanel id="maw-center" :default-size="57" :min-size="0">
      <slot v-if="$slots.center" name="center" />
      <Resizable v-else class="maw-column" direction="vertical">
        <ResizablePanel id="maw-viewport" :default-size="64" :min-size="0">
          <slot name="viewport" />
        </ResizablePanel>
        <ResizableHandle
          orientation="vertical"
          :label="t('studio.assetWorkbench.resizeBlueprint')"
        />
        <ResizablePanel id="maw-blueprint" :default-size="36" :min-size="0">
          <slot name="lower" />
        </ResizablePanel>
      </Resizable>
    </ResizablePanel>
    <ResizableHandle
      orientation="horizontal"
      :hit-area-margins="columnHandleMargins"
      :label="t('studio.assetWorkbench.resizeRight')"
    />
    <ResizablePanel id="maw-right" :default-size="26" :min-size="0">
      <Resizable
        v-if="$slots.inspector"
        direction="vertical"
        class="maw-column"
      >
        <ResizablePanel id="maw-right-tabs" :default-size="52" :min-size="0">
          <slot name="library" />
        </ResizablePanel>
        <ResizableHandle
          orientation="vertical"
          :label="t('studio.assetWorkbench.resizeInspector')"
        />
        <ResizablePanel
          id="maw-right-inspector"
          :default-size="48"
          :min-size="0"
        >
          <slot name="inspector" />
        </ResizablePanel>
      </Resizable>
      <slot v-else name="library" />
    </ResizablePanel>
  </Resizable>
</template>

<style scoped>
.maw-split {
  border-inline: 1px solid var(--border);
  flex: 1;
  min-height: 0;
}
.maw-split :deep([data-panel-size="0.0"]) {
  visibility: hidden;
}
.maw-column {
  height: 100%;
  min-height: 0;
}
</style>
