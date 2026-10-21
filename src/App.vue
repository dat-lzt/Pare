<script setup lang="ts">
import { computed, onMounted, reactive, ref, watch } from "vue";
import AboutPanel from "./components/AboutPanel.vue";
import AppIcon from "./components/AppIcon.vue";
import PreviewDialog from "./components/PreviewDialog.vue";
import SettingsPanel from "./components/SettingsPanel.vue";
import { useEngineGroups } from "./engineGroups";
import { languageSetting, locale, statusLabel, t, translateMessage } from "./i18n";
import { loadThumbnail } from "./thumbs";
import { useBatchStore } from "./stores/batch";
import { formatBytes, formatDelta, type Appearance, type FileRow, type Kind, type OutputFormat, type Preset, type Status } from "./types";
import markUrl from "./assets/mark.png";

const store = useBatchStore();

onMounted(() => {
  void store.init();
});

watch(
  () => store.settings.preset,
  (preset) => {
    if (preset === "lossless") store.settings.lossy = false;
  },
);

const formats = computed<Array<{ id: OutputFormat; label: string }>>(() => [
  { id: "original", label: t("formatOriginal") },
  { id: "webp", label: t("formatWebp") },
  { id: "jpeg", label: t("formatJpeg") },
  { id: "png", label: t("formatPng") },
]);

const converting = () => store.settings.outputFormat !== "original";

const presetCards = computed(() => [
  { id: "photo" as Preset, icon: "photo", label: t("presetPhoto"), detail: converting() ? t("presetPhotoConvert") : t("presetPhotoSame") },
  { id: "ui" as Preset, icon: "display", label: t("presetUi"), detail: converting() ? t("presetUiConvert") : t("presetUiSame") },
  {
    id: "lossless" as Preset,
    icon: "swap",
    label: converting() ? t("presetLossless") : t("presetLosslessSame"),
    detail: converting() ? t("presetLosslessConvert") : t("presetLosslessSameDetail"),
  },
]);

const currentDetail = computed(() => presetCards.value.find((item) => item.id === store.settings.preset)?.detail ?? "");

function chooseFormat(format: OutputFormat) {
  if (store.running) return;
  store.settings.outputFormat = format;
  void store.persist();
}

function choosePreset(preset: Preset) {
  if (store.running) return;
  store.settings.preset = preset;
  void store.persist();
}

function onStart(event: Event) {
  if ("button" in event && (event as PointerEvent).button !== 0) return;
  void store.start();
}

const waitingCount = computed(
  () => store.files.filter((file) => file.status === "waiting" && file.kind !== "other").length,
);
const inputTotal = computed(() => store.files.reduce((sum, file) => sum + file.inputBytes, 0));

function applyAppearance(mode: Appearance | undefined) {
  const root = document.documentElement;
  if (!mode || mode === "system") root.removeAttribute("data-theme");
  else root.setAttribute("data-theme", mode);
}

watch(() => store.settings.appearance, applyAppearance, { immediate: true });

watch(
  () => store.settings.language,
  (mode) => {
    languageSetting.value = mode || "system";
  },
  { immediate: true },
);

watch(locale, (value) => {
  document.documentElement.lang = value;
}, { immediate: true });

function statusIcon(status: Status) {
  if (status === "done" || status === "unchanged" || status === "first-frame") return "check";
  if (status === "failed") return "x";
  if (status === "running") return "spinner";
  if (status === "cancelled") return "minus";
  if (status === "unsupported") return "alert";
  return "clock";
}

function kindIcon(kind: Kind) {
  return kind === "other" ? "file" : "image";
}

const engineGroups = useEngineGroups();
const advancedOpen = ref(true);
const thumbSrc = reactive<Record<string, string>>({});

watch(
  () => store.files.map((file) => `${file.id}:${file.inputPath}`).join("\n"),
  () => {
    for (const file of store.files) {
      if (file.kind === "other" || thumbSrc[file.inputPath]) continue;
      void loadThumbnail(file.inputPath).then((url) => {
        if (url) thumbSrc[file.inputPath] = url;
      });
    }
  },
  { immediate: true },
);

const previewId = ref("");
const downloadable = computed(() => store.files.filter((file) => file.outputPath));
const allChecked = computed(
  () => downloadable.value.length > 0 && downloadable.value.every((file) => store.selected.includes(file.id)),
);

function openPreview(file: FileRow) {
  if (file.kind === "other") return;
  previewId.value = file.id;
}
</script>

<template>
  <main class="shell">
    <header class="top">
      <div class="brand">
        <img class="mark" :src="markUrl" alt="" />
        <strong>Pare</strong>
        <span>{{ t("privacy") }}</span>
      </div>
      <div class="actions">
        <button type="button" class="toolbar-btn icon-btn" @click="store.settingsOpen = true"><AppIcon name="gear" />{{ t("settings") }}</button>
        <button type="button" class="toolbar-btn icon-btn" @click="store.aboutOpen = true"><AppIcon name="info" />{{ t("about") }}</button>
      </div>
    </header>

    <div class="workspace">
      <div class="tasks">
        <section v-if="store.files.length === 0" class="panel drop" :class="{ over: store.dragging }">
          <div>
            <AppIcon class="icon-lg" name="image" />
            <h2>{{ t("dropTitle") }}</h2>
            <p class="hint">{{ t("dropHint") }}</p>
            <div class="actions" style="justify-content: center; margin-top: 12px">
              <button class="primary icon-btn" type="button" :disabled="!store.desktop" @click="store.pickFiles()"><AppIcon name="image" />{{ t("pickFiles") }}</button>
              <button class="icon-btn" type="button" :disabled="!store.desktop" @click="store.pickFolder()"><AppIcon name="folder" />{{ t("pickFolder") }}</button>
            </div>
            <p v-if="!store.desktop" class="warn">{{ t("browserWarn") }}</p>
          </div>
        </section>

        <template v-else>
          <section class="panel compact" :class="{ over: store.dragging }">
            <span>{{ t("dropMore") }}</span>
            <div class="actions">
              <button class="icon-btn" type="button" :disabled="!store.desktop" @click="store.pickFiles()"><AppIcon name="image" />{{ t("pickFiles") }}</button>
              <button class="icon-btn" type="button" :disabled="!store.desktop" @click="store.pickFolder()"><AppIcon name="folder" />{{ t("pickFolder") }}</button>
            </div>
          </section>
          <section class="panel table-wrap">
            <div class="list-head">
              <strong>{{ t("summary", { count: store.summary.total, size: formatBytes(inputTotal) }) }}</strong>
              <span v-if="store.summary.saved > 0" class="saved">{{ t("saved", { size: formatBytes(store.summary.saved) }) }}</span>
              <span v-else class="hint">{{ t("savedPending") }}</span>
            </div>
            <div class="table-scroll">
              <table>
                <thead>
                  <tr>
                    <th class="check">
                      <input
                        type="checkbox"
                        :title="t('checkTitle')"
                        :checked="allChecked"
                        :disabled="!downloadable.length"
                        @change="store.toggleAllDownloadable(($event.target as HTMLInputElement).checked)"
                      />
                    </th>
                    <th>{{ t("colFile") }}</th>
                    <th>{{ t("colInput") }}</th>
                    <th>{{ t("colResult") }}</th>
                    <th>{{ t("colDelta") }}</th>
                    <th>{{ t("colStatus") }}</th>
                    <th></th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="file in store.files" :key="file.id">
                    <td class="check">
                      <input
                        type="checkbox"
                        :disabled="!file.outputPath"
                        :checked="!!file.outputPath && store.selected.includes(file.id)"
                        @change="store.toggleSelected(file.id, ($event.target as HTMLInputElement).checked)"
                      />
                    </td>
                    <td>
                      <div class="file-line">
                        <button type="button" class="thumb-btn" :disabled="file.kind === 'other'" @click="openPreview(file)">
                          <img v-if="thumbSrc[file.inputPath]" :src="thumbSrc[file.inputPath]" alt="" />
                          <AppIcon v-else :name="kindIcon(file.kind)" />
                        </button>
                        <div class="file-name">
                          <button type="button" class="file-open" :disabled="file.kind === 'other'" :title="file.relativePath" @click="openPreview(file)">
                            {{ file.relativePath }}
                          </button>
                          <small>{{ file.kind }}<template v-if="file.note"> · {{ translateMessage(file.note) }}</template></small>
                        </div>
                      </div>
                    </td>
                    <td class="num">{{ formatBytes(file.inputBytes) }}</td>
                    <td class="num">{{ formatBytes(file.outputBytes) }}</td>
                    <td class="num" :class="{ down: (file.deltaRatio ?? 0) < 0, up: (file.deltaRatio ?? 0) > 0 }">
                      {{ formatDelta(file.deltaRatio) }}
                    </td>
                    <td><span class="status" :class="file.status"><AppIcon :name="statusIcon(file.status)" />{{ statusLabel(file.status) }}</span></td>
                    <td class="row-actions">
                      <button v-if="file.status === 'waiting' && file.kind !== 'other'" type="button" class="link" @click="store.startOne(file.id)">{{ t("rowStart") }}</button>
                      <button v-if="file.outputPath" type="button" class="link" @click="store.downloadOne(file.id)">{{ t("rowDownload") }}</button>
                      <button type="button" class="link" :disabled="file.status === 'running'" @click="store.remove(file.id)">{{ t("rowRemove") }}</button>
                    </td>
                  </tr>
                </tbody>
              </table>
            </div>
          </section>
        </template>
      </div>

      <aside class="panel side">
        <div class="row">
          <span>{{ t("format") }}</span>
          <div class="segment">
            <button
              v-for="format in formats"
              :key="format.id"
              type="button"
              :class="{ active: store.settings.outputFormat === format.id }"
              :disabled="store.running"
              @click="chooseFormat(format.id)"
            >
              {{ format.label }}
            </button>
          </div>
        </div>
        <div class="row">
          <span>{{ t("quality") }}</span>
          <div class="field">
            <div class="presets">
              <button
                v-for="preset in presetCards"
                :key="preset.id"
                type="button"
                :class="{ active: store.settings.preset === preset.id }"
                :disabled="store.running"
                @click="choosePreset(preset.id)"
              >
                <AppIcon :name="preset.icon" />
                {{ preset.label }}
              </button>
            </div>
            <p class="hint">{{ currentDetail }}</p>
          </div>
        </div>
        <div v-if="store.showLossy" class="row">
          <span>{{ t("lossy") }}</span>
          <div class="field">
            <label class="choice">
              <input
                v-model="store.settings.lossy"
                type="checkbox"
                :disabled="store.running || store.settings.preset === 'lossless'"
                @change="store.persist()"
              />
              <span>{{ store.settings.lossy ? t("on") : t("off") }}</span>
            </label>
            <p class="hint">{{ t("lossyHint") }}</p>
          </div>
        </div>
        <p v-if="store.jpegAlpha" class="warn">{{ t("warnAlpha") }}</p>
        <p v-if="store.pngFromJpeg" class="warn">{{ t("warnJpegPng") }}</p>
        <section class="advanced-block">
          <button type="button" class="advanced-toggle" @click="advancedOpen = !advancedOpen">
            {{ t("advanced") }}
            <AppIcon name="chevron" :class="{ open: advancedOpen }" />
          </button>
          <div v-show="advancedOpen">
            <template v-for="group in engineGroups" :key="group.title">
              <p class="group-label">{{ group.title }}</p>
              <label v-for="engine in group.items" :key="engine[0]" class="engine">
                <span>{{ engine[1] }}<small>{{ engine[2] }}</small></span>
                <input v-model="store.settings.engines[engine[0]]" class="switch" type="checkbox" @change="store.persist()" />
              </label>
            </template>
          </div>
        </section>
      </aside>
    </div>

    <footer class="actionbar">
      <span class="bar-label icon-btn"><AppIcon name="folder" />{{ t("outputTo") }}</span>
      <div class="path" :title="store.outputDir">{{ store.outputDir || t("outputMissing") }}</div>
      <button type="button" :disabled="store.running || !store.desktop" @click="store.chooseOutput()">{{ t("pickFolder") }}</button>
      <button type="button" :disabled="!store.outputDir || !store.desktop" @click="store.openOutput()">{{ t("open") }}</button>
      <button type="button" :disabled="!store.selected.length" @click="store.downloadZip()">{{ t("zip") }}</button>
      <button type="button" :disabled="store.running || !store.files.length" @click="store.clear()">{{ t("clear") }}</button>
      <button v-if="store.running" type="button" @click="store.cancel()">{{ t("cancel") }}</button>
      <button type="button" class="primary icon-btn" :disabled="!store.canStart" @pointerup="onStart" @click="onStart">
        <AppIcon name="play" />{{ waitingCount ? t("startCount", { count: waitingCount }) : t("start") }}
      </button>
      <p v-if="store.outputHint" class="warn bar-note">{{ translateMessage(store.outputHint) }}</p>
      <p v-if="store.outputError" class="error bar-note">{{ translateMessage(store.outputError) }}</p>
    </footer>

    <PreviewDialog
      v-if="previewId"
      :files="store.files"
      :current-id="previewId"
      @close="previewId = ''"
      @update:current-id="previewId = $event"
    />

    <SettingsPanel v-if="store.settingsOpen" />
    <AboutPanel v-if="store.aboutOpen" />
  </main>
</template>
