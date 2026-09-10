<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import { useI18n } from "vue-i18n";
import FMarkdown from "@/components/markdown/FMarkdown.vue";

/**
 * 知识库阅读页：按路由 topic 从 assets/knowledge 载入双语文档
 * （zh-CN / en-US 随当前 locale 切换），FMarkdown 渲染。
 */

const KNOWLEDGE_TOPICS = ["file-types", "rendering", "engine"] as const;
type KnowledgeTopic = (typeof KNOWLEDGE_TOPICS)[number];

const documents = import.meta.glob<string>("../../assets/knowledge/*.md", {
  eager: true,
  import: "default",
  query: "?raw",
}) as unknown as Record<string, string>;

const route = useRoute();
const { locale } = useI18n();

const topic = computed<KnowledgeTopic>(() => {
  const value = route.params.topic as string | undefined;
  return (KNOWLEDGE_TOPICS as readonly string[]).includes(value ?? "")
    ? (value as KnowledgeTopic)
    : "file-types";
});

const source = computed(() => {
  const lang = locale.value.startsWith("zh") ? "zh-CN" : "en-US";
  return (
    documents[`../../assets/knowledge/${topic.value}.${lang}.md`] ??
    documents[`../../assets/knowledge/${topic.value}.zh-CN.md`] ??
    ""
  );
});
</script>

<template>
  <section class="knowledge-page">
    <FMarkdown :source="source" />
  </section>
</template>

<style scoped>
.knowledge-page {
  margin: 0 auto;
  max-width: 860px;
  padding: 18px 20px 40px;
}
</style>
