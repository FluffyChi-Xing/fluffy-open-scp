<script setup lang="ts">
import { SwitchRoot, SwitchThumb } from "reka-ui";
import { cn } from "@/lib/utils";

interface Props {
  class?: string;
  disabled?: boolean;
  id?: string;
  ariaLabel?: string;
}

const props = defineProps<Props>();
const model = defineModel<boolean>({ default: false });
</script>

<template>
  <SwitchRoot
    :id="props.id"
    v-model="model"
    :disabled="props.disabled"
    :aria-label="props.ariaLabel"
    :class="cn(
      'peer inline-flex h-5 w-9 shrink-0 cursor-pointer items-center rounded-full border border-border bg-muted p-0.5 shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background data-[state=checked]:border-primary data-[state=checked]:bg-primary disabled:cursor-not-allowed disabled:opacity-50',
      props.class,
    )"
  >
    <SwitchThumb class="pointer-events-none block size-4 rounded-full bg-background shadow-sm transition-transform data-[state=checked]:translate-x-4" />
  </SwitchRoot>
</template>
