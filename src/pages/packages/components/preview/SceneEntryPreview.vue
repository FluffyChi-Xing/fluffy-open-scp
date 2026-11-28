<script setup lang="ts">
import ActorModelPreview from "./ActorModelPreview.vue";
import type { SceneInfo } from "@/api/property-semantics";
import { keyText } from "./semanticCard";
defineProps<{ scene: SceneInfo; packageId: number }>();
const hex = (id: number) =>
  `0x${id.toString(16).padStart(8, "0").toUpperCase()}`;
</script>
<template>
  <section class="scene-preview">
    <h4>{{ $t("package.scene.title") }}</h4>
    <p>{{ $t("package.scene.hint") }}</p>
    <p v-if="!scene.recognized">{{ $t("package.scene.unknown") }}</p>
    <p v-else>{{ $t("package.scene.count", { n: scene.actors.length }) }}</p>
    <p v-if="scene.selfDestruct != null">
      {{ $t("package.scene.selfDestruct") }}:
      {{ $t(scene.selfDestruct ? "package.scene.yes" : "package.scene.no") }}
    </p>
    <p v-for="(issue, i) in scene.issues" :key="i" role="status">
      {{
        $t("package.scene.columnIssue", {
          property: hex(issue.property),
          expected: issue.expected,
          actual: issue.actual,
        })
      }}
    </p>
    <details v-for="actor in scene.actors" :key="actor.id" class="actor">
      <summary>
        {{ $t("package.scene.actor") }} {{ hex(actor.id) }} ·
        {{
          $t("package.scene.modelCount", {
            n: actor.models.length,
            t: actor.templates.length,
          })
        }}
      </summary>
      <ActorModelPreview
        :package-id="packageId"
        :actor="actor"
        :references="[...actor.models, ...actor.templates]"
      />
      <dl>
        <template
          v-for="field in ['models', 'templates'] as const"
          :key="field"
        >
          <dt>{{ $t(`package.scene.${field}`) }}</dt>
          <dd v-if="!actor[field].length">{{ $t("package.scene.absent") }}</dd>
          <dd v-for="(key, i) in actor[field]" :key="i">
            <code>{{ keyText(key) }}</code>
          </dd>
        </template>
        <template v-for="field in ['speed', 'scale'] as const" :key="field"
          ><dt>{{ $t(`package.scene.${field}`) }}</dt>
          <dd>{{ actor[field] ?? $t("package.scene.absent") }}</dd></template
        >
        <template
          v-for="field in [
            'animation',
            'spawn',
            'destroy',
            'resourceBind',
          ] as const"
          :key="field"
          ><dt>{{ $t(`package.scene.${field}`) }}</dt>
          <dd>
            <code>{{
              actor[field] ? keyText(actor[field]!) : $t("package.scene.absent")
            }}</code>
          </dd></template
        >
      </dl>
      <div class="table-scroll">
        <table v-if="actor.appearances.length">
          <caption>
            {{
              $t("package.scene.appearance")
            }}
          </caption>
          <thead>
            <tr>
              <th
                v-for="field in [
                  'body',
                  'skin',
                  'skinMax',
                  'outfit',
                  'outfitMax',
                ] as const"
                :key="field"
                scope="col"
              >
                {{ $t(`package.scene.${field}`) }}
              </th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(appearance, i) in actor.appearances" :key="i">
              <td
                v-for="field in [
                  'body',
                  'skin',
                  'skinMax',
                  'outfit',
                  'outfitMax',
                ] as const"
                :key="field"
              >
                {{ appearance[field] ?? $t("package.scene.absent") }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </details>
  </section>
</template>
<style scoped>
.scene-preview {
  display: grid;
  gap: 12px;
  min-width: 0;
}
p {
  color: var(--muted-foreground);
  font-size: 12px;
}
.actor {
  border-top: 1px solid var(--border);
  padding-block: 10px;
}
summary {
  cursor: pointer;
  font-size: 13px;
}
dl {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  gap: 8px 16px;
  font-size: 12px;
  padding-block: 12px;
}
dd {
  margin: 0;
  overflow-wrap: anywhere;
}
.table-scroll {
  overflow-x: auto;
}
table {
  width: 100%;
  font-size: 12px;
  border-collapse: collapse;
}
th,
td {
  padding: 8px;
  text-align: start;
  border: 1px solid var(--border);
}
caption {
  text-align: start;
  padding-block: 8px;
}
h4,
p {
  margin: 0;
}
h4 {
  font-size: 12px;
  font-weight: 600;
}
section,
details,
dl,
dd {
  min-width: 0;
}
summary {
  line-height: 1.7;
  overflow-wrap: anywhere;
  border-radius: var(--radius-sm);
}
summary:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 3px;
}
code {
  overflow-wrap: anywhere;
}
dl {
  grid-template-columns: minmax(80px, 0.4fr) minmax(0, 1fr);
}
</style>
