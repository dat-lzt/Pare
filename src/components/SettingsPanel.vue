<script setup lang="ts">
import { useEngineGroups } from "../engineGroups";
import { t, type MessageKey } from "../i18n";
import { useBatchStore } from "../stores/batch";
import type { Appearance, Language } from "../types";

const store = useBatchStore();

const languages: Array<{ id: Language; label: MessageKey }> = [
  { id: "system", label: "langSystem" },
  { id: "zh-CN", label: "langZh" },
  { id: "en", label: "langEn" },
];

const appearances: Array<{ id: Appearance; label: MessageKey }> = [
  { id: "system", label: "themeSystem" },
  { id: "light", label: "themeLight" },
  { id: "dark", label: "themeDark" },
];

const groups = useEngineGroups();

function setAppearance(mode: Appearance) {
  store.settings.appearance = mode;
  void store.persist();
}

function setLanguage(mode: Language) {
  store.settings.language = mode;
  void store.persist();
}

function close() {
  void store.persist();
  store.settingsOpen = false;
}
</script>

<template>
  <div class="overlay" @click.self="close">
    <div class="sheet panel settings-sheet">
      <h3>{{ t("settings") }}</h3>
      <p class="hint">{{ t("settingsHint") }}</p>
      <div class="settings-body">

      <section>
        <h3>{{ t("appearance") }}</h3>
        <p class="group-label">{{ t("language") }}</p>
        <div class="segment">
          <button
            v-for="item in languages"
            :key="item.id"
            type="button"
            :class="{ active: store.settings.language === item.id }"
            @click="setLanguage(item.id)"
          >
            {{ t(item.label) }}
          </button>
        </div>
        <p class="group-label">{{ t("theme") }}</p>
        <div class="segment">
          <button
            v-for="item in appearances"
            :key="item.id"
            type="button"
            :class="{ active: store.settings.appearance === item.id }"
            @click="setAppearance(item.id)"
          >
            {{ t(item.label) }}
          </button>
        </div>
      </section>

      <section>
        <h3>{{ t("preferences") }}</h3>
        <label class="toggle">
          <span>{{ t("concurrency", { count: store.settings.concurrency }) }}<small>{{ t("concurrencyHint") }}</small></span>
          <input v-model.number="store.settings.concurrency" type="range" min="1" max="8" @change="store.persist()" />
        </label>
        <label class="toggle">
          <span>{{ t("gifFirst") }}<small>{{ t("gifFirstHint") }}</small></span>
          <input v-model="store.settings.gifFirstFrame" type="checkbox" @change="store.persist()" />
        </label>
      </section>

      <section>
        <h3>{{ t("engines") }}</h3>
        <template v-for="group in groups" :key="group.title">
          <p class="group-label">{{ group.title }}</p>
          <label v-for="engine in group.items" :key="engine[0]" class="engine">
            <span>{{ engine[1] }}<small>{{ engine[2] }}</small></span>
            <input v-model="store.settings.engines[engine[0]]" type="checkbox" @change="store.persist()" />
          </label>
        </template>
      </section>

      <section>
        <h3>{{ t("debug") }}</h3>
        <label class="toggle">
          <span>{{ t("debugLog") }}<small>{{ t("debugHint") }}</small></span>
          <input v-model="store.settings.debugLog" type="checkbox" @change="store.persist()" />
        </label>
      </section>
      </div>

      <div class="actions settings-foot">
        <button type="button" class="primary" @click="close">{{ t("done") }}</button>
      </div>
    </div>
  </div>
</template>
