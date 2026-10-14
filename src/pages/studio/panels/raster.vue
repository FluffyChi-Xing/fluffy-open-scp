<script setup lang="ts">
import RasterEditor from "@/components/raster/RasterEditor.vue";
</script>
<template><RasterEditor /></template>

<style>
.page-content:has(.raster-editor) {
  height: calc(100dvh - 56px);
  min-height: 0;
  overflow: hidden;
}
.page-content:has(.raster-editor) .content-frame,
.page-content:has(.raster-editor) .route-page {
  flex: 1;
  min-height: 0;
  height: 100%;
}
.page-content:has(.raster-editor) .content-frame {
  padding: 0;
}
.page-content:has(.raster-editor) .footer {
  display: none;
}
@media (max-width: 700px) {
  .page-content:has(.raster-editor) {
    height: auto;
    min-height: calc(100dvh - 56px);
    overflow: visible;
  }
}
</style>
