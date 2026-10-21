<script setup lang="ts">
import { onMounted, ref } from "vue";
import { engineVersions } from "../api";
import { t } from "../i18n";
import { useBatchStore } from "../stores/batch";
import type { EngineVersion } from "../types";
import markUrl from "../assets/mark.png";

const store = useBatchStore();
const versions = ref<EngineVersion[]>([]);

onMounted(async () => {
  if (!store.desktop) return;
  versions.value = await engineVersions();
});
</script>

<template>
  <div class="overlay" @click.self="store.aboutOpen = false">
    <div class="sheet panel">
      <img class="about-mark" :src="markUrl" alt="" />
      <h3>Pare</h3>
      <p>{{ t("aboutVersion", { version: "0.1.0" }) }}</p>
      <p>{{ t("aboutBody") }}</p>
      <section>
        <div v-for="engine in versions" :key="engine.name" class="engine">
          <span>{{ engine.name }}<small>{{ engine.version }}</small></span>
        </div>
        <p v-if="!versions.length" class="hint">{{ t("aboutEnginesEmpty") }}</p>
      </section>
      <div class="actions" style="justify-content: flex-end">
        <button type="button" class="primary" @click="store.aboutOpen = false">{{ t("close") }}</button>
      </div>
    </div>
  </div>
</template>
