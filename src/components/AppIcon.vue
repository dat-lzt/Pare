<script setup lang="ts">
defineProps<{ name: string }>();
</script>

<template>
  <svg class="icon" :class="{ spin: name === 'spinner' }" viewBox="0 0 16 16" aria-hidden="true">
    <template v-if="name === 'image' || name === 'photo'">
      <rect x="1.5" y="2.5" width="13" height="11" rx="1.6" />
      <circle cx="5.4" cy="6" r="1.1" />
      <path d="M2.2 11.2 5.4 8.2l2.3 2.1 2.1-1.9 4 3" />
    </template>
    <template v-else-if="name === 'folder'">
      <path d="M1.6 4.2h4.2L7.2 2.7h7.2v9.6H1.6z" />
    </template>
    <template v-else-if="name === 'gear'">
      <circle cx="8" cy="8" r="2" />
      <path d="M8 1.6v1.7M8 12.7v1.7M1.6 8h1.7M12.7 8h1.7M3.3 3.3l1.2 1.2M11.5 11.5l1.2 1.2M12.7 3.3l-1.2 1.2M4.5 11.5l-1.2 1.2" />
    </template>
    <template v-else-if="name === 'info'">
      <circle cx="8" cy="8" r="6.1" />
      <path d="M8 7.2V11.4M8 4.8h.01" />
    </template>
    <template v-else-if="name === 'display'">
      <rect x="1.6" y="2.4" width="12.8" height="8.4" rx="1.3" />
      <path d="M6 13.4h4M8 10.8v2.6" />
    </template>
    <template v-else-if="name === 'swap'">
      <path d="M2.2 5.2h9.2L9.2 3M13.8 10.8H4.6L6.8 13" />
    </template>
    <template v-else-if="name === 'play'">
      <path d="M5.2 3.2v9.6L12.6 8z" />
    </template>
    <template v-else-if="name === 'check'">
      <path d="M3 8.4 6.2 11.6 13 4.2" />
    </template>
    <template v-else-if="name === 'x'">
      <circle cx="8" cy="8" r="6.1" />
      <path d="M5.4 5.4 10.6 10.6M10.6 5.4 5.4 10.6" />
    </template>
    <template v-else-if="name === 'alert'">
      <circle cx="8" cy="8" r="6.1" />
      <path d="M8 4.8v4.2M8 11.2h.01" />
    </template>
    <template v-else-if="name === 'clock'">
      <circle cx="8" cy="8" r="6.1" />
      <path d="M8 4.8V8l2.2 1.4" />
    </template>
    <template v-else-if="name === 'minus'">
      <circle cx="8" cy="8" r="6.1" />
      <path d="M5 8h6" />
    </template>
    <template v-else-if="name === 'spinner'">
      <circle cx="8" cy="8" r="5.4" stroke-dasharray="18 12" />
    </template>
    <template v-else-if="name === 'plus'">
      <path d="M8 3.2v9.6M3.2 8h9.6" />
    </template>
    <template v-else-if="name === 'chevron'">
      <path d="M4 6.2 8 10.2 12 6.2" />
    </template>
    <template v-else-if="name === 'download'">
      <path d="M8 2.4v7.2M5.2 7.2 8 10.2l2.8-3M3 13.2h10" />
    </template>
    <template v-else>
      <path d="M4 1.6h5.2L12.4 4.8v9.6H4z" />
      <path d="M9.2 1.8V5h3" />
    </template>
  </svg>
</template>
