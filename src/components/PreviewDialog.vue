<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import * as api from "../api";
import { t, translateMessage } from "../i18n";
import { formatBytes, type FileRow } from "../types";
import AppIcon from "./AppIcon.vue";

const props = defineProps<{ files: FileRow[]; currentId: string }>();
const emit = defineEmits<{ close: []; "update:currentId": [id: string] }>();

const viewable = computed(() => props.files.filter((file) => file.kind !== "other"));
const index = computed(() => viewable.value.findIndex((file) => file.id === props.currentId));
const file = computed(() => (index.value >= 0 ? viewable.value[index.value] : undefined));
const previewPath = computed(() => file.value?.outputPath || file.value?.inputPath || "");

const src = ref("");
const error = ref("");
const loading = ref(false);
const naturalW = ref(0);
const naturalH = ref(0);
const mode = ref<"fit" | "manual">("fit");
const scale = ref(1);
const panX = ref(0);
const panY = ref(0);
const stageTick = ref(0);
const panning = ref(false);
const backdrop = ref<"checker" | "dark" | "light">("checker");
const stage = ref<HTMLElement | null>(null);
const dialog = ref<HTMLElement | null>(null);

let request = 0;
let dragging = false;
let dragStartX = 0;
let dragStartY = 0;
let panStartX = 0;
let panStartY = 0;
let observer: ResizeObserver | undefined;

const hasPrev = computed(() => index.value > 0);
const hasNext = computed(() => index.value >= 0 && index.value < viewable.value.length - 1);
const indexLabel = computed(() =>
  t("previewIndex", { current: Math.max(index.value, 0) + 1, total: viewable.value.length }),
);

function extension(path: string) {
  const name = path.split(/[/\\]/).pop() ?? path;
  const dot = name.lastIndexOf(".");
  return dot >= 0 ? name.slice(dot + 1).toUpperCase() : "";
}

const meta = computed(() => {
  const current = file.value;
  if (!current) return "";
  const path = previewPath.value;
  const format = extension(path);
  const bytes =
    current.outputPath && path === current.outputPath && current.outputBytes != null
      ? current.outputBytes
      : current.inputBytes;
  const parts = [format, formatBytes(bytes)].filter(Boolean);
  if (naturalW.value > 0 && naturalH.value > 0) parts.push(`${naturalW.value}×${naturalH.value}`);
  const lower = format.toLowerCase();
  if (current.hasAlpha && lower !== "jpg" && lower !== "jpeg") parts.push(t("previewTransparent"));
  return parts.join(" · ");
});

function stageSize() {
  const el = stage.value;
  if (!el) return { w: 0, h: 0 };
  return { w: el.clientWidth, h: el.clientHeight };
}

function fitScale() {
  void stageTick.value;
  if (!naturalW.value || !naturalH.value) return 1;
  const { w, h } = stageSize();
  if (w < 2 || h < 2) return 1;
  return Math.min(1, w / naturalW.value, h / naturalH.value);
}

function displayScale() {
  return mode.value === "fit" ? fitScale() : scale.value;
}

const percentLabel = computed(() => (naturalW.value ? `${Math.round(displayScale() * 100)}%` : "—"));

const canPan = computed(() => {
  if (!naturalW.value || !naturalH.value) return false;
  const zoom = displayScale();
  const { w, h } = stageSize();
  return naturalW.value * zoom > w + 1 || naturalH.value * zoom > h + 1;
});

const imgStyle = computed(() => ({
  transform: `translate(calc(-50% + ${panX.value}px), calc(-50% + ${panY.value}px)) scale(${displayScale()})`,
}));

const backdropLabel = computed(() => {
  if (backdrop.value === "dark") return t("previewDark");
  if (backdrop.value === "light") return t("previewLight");
  return t("previewChecker");
});

function resetView() {
  mode.value = "fit";
  scale.value = 1;
  panX.value = 0;
  panY.value = 0;
  naturalW.value = 0;
  naturalH.value = 0;
}

async function load() {
  const current = file.value;
  const path = previewPath.value;
  const mine = ++request;
  src.value = "";
  error.value = "";
  loading.value = false;
  resetView();
  if (!current || !path) {
    error.value = t("errPreview");
    return;
  }
  loading.value = true;
  try {
    const image = await api.readPreview(path);
    if (mine !== request) return;
    src.value = `data:${image.mime};base64,${image.base64}`;
  } catch (err) {
    if (mine !== request) return;
    error.value = err instanceof Error ? err.message : String(err);
  } finally {
    if (mine === request) loading.value = false;
  }
}

function onLoad(event: Event) {
  const img = event.target as HTMLImageElement;
  naturalW.value = img.naturalWidth || 0;
  naturalH.value = img.naturalHeight || 0;
  mode.value = "fit";
  panX.value = 0;
  panY.value = 0;
}

function fit() {
  mode.value = "fit";
  panX.value = 0;
  panY.value = 0;
}

function actual() {
  mode.value = "manual";
  scale.value = 1;
  panX.value = 0;
  panY.value = 0;
}

function zoomBy(factor: number, origin?: { x: number; y: number }) {
  const current = displayScale() || 1;
  const next = Math.min(8, Math.max(0.05, current * factor));
  if (origin && stage.value) {
    const rect = stage.value.getBoundingClientRect();
    const mx = origin.x - rect.left - rect.width / 2;
    const my = origin.y - rect.top - rect.height / 2;
    panX.value = mx - ((mx - panX.value) * next) / current;
    panY.value = my - ((my - panY.value) * next) / current;
  }
  mode.value = "manual";
  scale.value = next;
}

function onWheel(event: WheelEvent) {
  if (!src.value || !naturalW.value) return;
  zoomBy(event.deltaY < 0 ? 1.1 : 1 / 1.1, { x: event.clientX, y: event.clientY });
}

function onPointerDown(event: PointerEvent) {
  if (!canPan.value || event.button !== 0) return;
  dragging = true;
  panning.value = true;
  dragStartX = event.clientX;
  dragStartY = event.clientY;
  panStartX = panX.value;
  panStartY = panY.value;
  (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
}

function onPointerMove(event: PointerEvent) {
  if (!dragging) return;
  panX.value = panStartX + (event.clientX - dragStartX);
  panY.value = panStartY + (event.clientY - dragStartY);
}

function onPointerUp() {
  dragging = false;
  panning.value = false;
}

function go(step: number) {
  const next = viewable.value[index.value + step];
  if (next) emit("update:currentId", next.id);
}

function cycleBackdrop() {
  const order = ["checker", "dark", "light"] as const;
  backdrop.value = order[(order.indexOf(backdrop.value) + 1) % order.length];
}

function onKey(event: KeyboardEvent) {
  const target = event.target as HTMLElement | null;
  if (target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable)) return;
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  } else if (event.key === "ArrowLeft") {
    event.preventDefault();
    go(-1);
  } else if (event.key === "ArrowRight") {
    event.preventDefault();
    go(1);
  } else if (event.key === "0") {
    event.preventDefault();
    fit();
  } else if (event.key === "1") {
    event.preventDefault();
    actual();
  }
}

watch(() => props.currentId, () => void load(), { immediate: true });
watch(file, (value) => {
  if (!value) emit("close");
});

onMounted(() => {
  window.addEventListener("keydown", onKey);
  dialog.value?.focus();
  if (stage.value) {
    observer = new ResizeObserver(() => {
      stageTick.value += 1;
    });
    observer.observe(stage.value);
  }
});

onUnmounted(() => {
  request += 1;
  window.removeEventListener("keydown", onKey);
  observer?.disconnect();
});
</script>

<template>
  <div class="preview-overlay" @click.self="emit('close')">
    <div ref="dialog" class="preview-frame" role="dialog" aria-modal="true" tabindex="-1">
      <header class="preview-head">
        <div class="preview-title">
          <strong :title="file?.relativePath">{{ file?.relativePath }}</strong>
          <span>{{ meta }}</span>
        </div>
        <span class="preview-index">{{ indexLabel }}</span>
        <button type="button" class="icon-quiet" :title="t('previewPrev')" :disabled="!hasPrev" @click="go(-1)">
          <AppIcon class="turn-left" name="chevron" />
        </button>
        <button type="button" class="icon-quiet" :title="t('previewNext')" :disabled="!hasNext" @click="go(1)">
          <AppIcon class="turn-right" name="chevron" />
        </button>
        <button type="button" class="icon-quiet" :title="t('close')" @click="emit('close')">
          <AppIcon name="x" />
        </button>
      </header>

      <div
        ref="stage"
        class="preview-canvas"
        :class="[backdrop, { 'can-pan': canPan && !panning, panning }]"
        @wheel.prevent="onWheel"
        @dblclick="fit"
        @pointerdown="onPointerDown"
        @pointermove="onPointerMove"
        @pointerup="onPointerUp"
        @pointercancel="onPointerUp"
      >
        <p v-if="error" class="preview-status error">{{ translateMessage(error) }}</p>
        <p v-else-if="loading || !src" class="preview-status hint">{{ t("previewLoading") }}</p>
        <img v-if="src" :src="src" :style="imgStyle" alt="" draggable="false" @load="onLoad" />
      </div>

      <footer class="preview-foot">
        <div class="preview-zoom">
          <button type="button" class="icon-quiet" :disabled="!naturalW" :title="t('previewZoomOut')" @click="zoomBy(1 / 1.25)">−</button>
          <span class="preview-zoom-label">{{ percentLabel }}</span>
          <button type="button" class="icon-quiet" :disabled="!naturalW" :title="t('previewZoomIn')" @click="zoomBy(1.25)">+</button>
        </div>
        <div class="preview-tools">
          <button type="button" :disabled="!naturalW" @click="fit">{{ t("previewFit") }}</button>
          <button type="button" :disabled="!naturalW" @click="actual">{{ t("previewActual") }}</button>
          <button type="button" @click="cycleBackdrop">{{ backdropLabel }}</button>
        </div>
      </footer>
    </div>
  </div>
</template>
