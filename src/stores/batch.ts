import { defineStore } from "pinia";
import { computed, ref } from "vue";
import * as api from "../api";
import type { FileRow, FileUpdate, Settings } from "../types";

const defaultSettings = (): Settings => ({
  outputFormat: "webp",
  preset: "photo",
  lossy: false,
  engines: {
    oxipng: true,
    pngquant: true,
    zopflipng: true,
    jpegtran: true,
    jpegoptim: true,
    gifsicle: true,
    svgcleaner: true,
  },
  concurrency: 4,
  gifFirstFrame: false,
  debugLog: false,
  appearance: "system",
  language: "system",
});

export const useBatchStore = defineStore("batch", () => {
  const files = ref<FileRow[]>([]);
  const roots = ref<string[]>([]);
  const settings = ref<Settings>(defaultSettings());
  const outputDir = ref("");
  const outputTouched = ref(false);
  const outputHint = ref("");
  const outputError = ref("");
  const running = ref(false);
  const dragging = ref(false);
  const settingsOpen = ref(false);
  const aboutOpen = ref(false);
  const desktop = ref(api.inTauri());

  const processable = computed(() => files.value.some((file) => file.status === "waiting" && file.kind !== "other"));
  const canStart = computed(() => processable.value && outputDir.value.trim() !== "" && !outputError.value && !running.value);

  const jpegAlpha = computed(
    () => settings.value.outputFormat === "jpeg" && files.value.some((file) => file.hasAlpha || file.kind === "svg"),
  );
  const pngFromJpeg = computed(
    () => settings.value.outputFormat === "png" && files.value.some((file) => file.kind === "jpeg"),
  );
  const showLossy = computed(() => settings.value.outputFormat === "original");

  const summary = computed(() => {
    const counts = { done: 0, unchanged: 0, failed: 0, waiting: 0, saved: 0 };
    for (const file of files.value) {
      if (file.status === "done") counts.done += 1;
      if (file.status === "unchanged") counts.unchanged += 1;
      if (file.status === "failed") counts.failed += 1;
      if (file.status === "waiting") counts.waiting += 1;
      if (file.status === "done" && file.deltaRatio != null && file.deltaRatio < 0 && file.outputBytes != null) {
        counts.saved += file.inputBytes - file.outputBytes;
      }
    }
    return { total: files.value.length, ...counts };
  });

  function snapshotSettings(): Settings {
    const next = JSON.parse(JSON.stringify(settings.value)) as Settings;
    if (next.preset === "lossless") {
      next.lossy = false;
    }
    next.concurrency = Math.min(8, Math.max(1, next.concurrency));
    return next;
  }

  async function refreshValidation() {
    if (!desktop.value || !outputDir.value) {
      outputError.value = outputDir.value ? "" : "";
      return;
    }
    try {
      await api.validateOutput(outputDir.value, roots.value);
      outputError.value = "";
    } catch (error) {
      outputError.value = error instanceof Error ? error.message : String(error);
    }
  }

  async function addPaths(paths: string[]) {
    if (!desktop.value) return;
    const inspected = await api.inspectPaths(paths);
    const known = new Set(files.value.map((file) => file.inputPath));
    for (const file of inspected.files) {
      if (known.has(file.inputPath)) continue;
      const row: FileRow = {
        ...file,
        id: crypto.randomUUID(),
        status: file.kind === "other" ? "unsupported" : "waiting",
        outputBytes: null,
        deltaRatio: null,
        outputPath: null,
        note: file.note || (file.kind === "other" ? `不支持 .${file.inputPath.split(".").pop() ?? ""}` : ""),
      };
      files.value.push(row);
      if (running.value && row.status === "waiting") {
        try {
          await api.enqueueJobs([toJob(row)]);
        } catch {
          row.status = "waiting";
        }
      }
    }
    roots.value = Array.from(new Set([...roots.value, ...inspected.roots]));
    const outputWasEmpty = !outputDir.value.trim();
    if (!outputTouched.value && inspected.suggestedOutput) {
      outputDir.value = inspected.suggestedOutput;
      if (outputWasEmpty) {
        outputHint.value = `还没选择输出文件夹，结果会写到旁边的「${baseName(inspected.suggestedOutput)}」。`;
      }
    } else if (!outputTouched.value && outputWasEmpty) {
      try {
        const fallback = await api.defaultOutput();
        outputDir.value = fallback;
        outputHint.value = fallback ? `还没选择输出文件夹，结果会写到「${fallback}」。` : "";
      } catch {
        outputHint.value = "";
      }
    }
    await refreshValidation();
    if (!outputDir.value.trim()) outputError.value = "请选择输出文件夹";
  }

  function baseName(path: string) {
    const parts = path.split(/[/\\]/).filter(Boolean);
    return parts[parts.length - 1] || path;
  }

  function toJob(file: FileRow) {
    return {
      id: file.id,
      inputPath: file.inputPath,
      relativePath: file.relativePath,
      kind: file.kind,
      animated: file.animated,
      inputBytes: file.inputBytes,
    };
  }

  function applyUpdate(update: FileUpdate) {
    const row = files.value.find((file) => file.id === update.id);
    if (!row) return;
    row.status = update.status;
    row.outputBytes = update.outputBytes;
    row.deltaRatio = update.deltaRatio;
    row.outputPath = update.outputPath;
    row.note = update.note;
  }

  async function start(onlyId?: string) {
    const jobs = files.value
      .filter((file) => file.status === "waiting" && file.kind !== "other" && (!onlyId || file.id === onlyId))
      .map(toJob);
    if (!outputDir.value.trim() || jobs.length === 0) {
      outputError.value = outputDir.value.trim() ? "没有可处理的图片" : "请选择输出文件夹";
      return;
    }
    outputError.value = "";
    outputHint.value = "";
    if (running.value) {
      try {
        await api.enqueueJobs(jobs);
      } catch (error) {
        outputError.value = error instanceof Error ? error.message : String(error);
      }
      return;
    }
    running.value = true;
    try {
      await api.startBatch({
        jobs,
        settings: snapshotSettings(),
        outputDir: outputDir.value,
        roots: roots.value,
      });
    } catch (error) {
      running.value = false;
      outputError.value = error instanceof Error ? error.message : String(error);
    }
  }

  const selected = ref<string[]>([]);

  function toggleSelected(id: string, on: boolean) {
    selected.value = on ? Array.from(new Set([...selected.value, id])) : selected.value.filter((item) => item !== id);
  }

  function toggleAllDownloadable(on: boolean) {
    selected.value = on ? files.value.filter((file) => file.outputPath).map((file) => file.id) : [];
  }

  function downloadName(file: FileRow) {
    const path = file.outputPath ?? "";
    const base = path.split(/[/\\]/).pop() || file.relativePath.split(/[/\\]/).pop() || "image";
    return base;
  }

  async function downloadOne(id: string) {
    const file = files.value.find((item) => item.id === id);
    if (!file?.outputPath) return;
    const name = downloadName(file);
    const ext = name.split(".").pop() || "bin";
    const dest = await api.pickSave(name, ext);
    if (!dest) return;
    try {
      await api.saveCopy(file.outputPath, dest);
    } catch (error) {
      outputError.value = error instanceof Error ? error.message : String(error);
    }
  }

  async function downloadZip() {
    const chosen = files.value.filter((file) => selected.value.includes(file.id) && file.outputPath);
    if (!chosen.length) return;
    const dest = await api.pickSave("Pare.zip", "zip");
    if (!dest) return;
    try {
      await api.exportZip(
        dest,
        chosen.map((file) => ({ path: file.outputPath as string, name: downloadName(file) })),
      );
    } catch (error) {
      outputError.value = error instanceof Error ? error.message : String(error);
    }
  }

  async function cancel() {
    await api.cancelBatch();
  }

  async function remove(id: string) {
    const row = files.value.find((file) => file.id === id);
    if (!row || row.status === "running") return;
    if (row.status === "waiting") await api.removeWaiting(id).catch(() => undefined);
    files.value = files.value.filter((file) => file.id !== id);
    selected.value = selected.value.filter((item) => item !== id);
  }

  function clear() {
    if (running.value) return;
    files.value = [];
    roots.value = [];
    selected.value = [];
  }

  async function chooseOutput() {
    const picked = await api.pickDirectory();
    if (!picked) return;
    outputDir.value = picked;
    outputTouched.value = true;
    outputHint.value = "";
    await refreshValidation();
  }

  async function openOutput() {
    if (!outputDir.value) return;
    outputError.value = "";
    try {
      await api.revealDirectory(outputDir.value);
    } catch (error) {
      outputError.value = error instanceof Error ? error.message : String(error);
    }
  }

  async function persist() {
    if (!desktop.value) return;
    await api.saveSettings(snapshotSettings());
  }

  async function init() {
    desktop.value = api.inTauri();
    if (!desktop.value) {
      outputDir.value = "Pare Output";
      return;
    }
    const [loaded, output] = await Promise.all([api.loadSettings(), api.defaultOutput()]);
    settings.value = {
      ...defaultSettings(),
      ...loaded,
      engines: { ...defaultSettings().engines, ...loaded.engines },
      appearance: loaded.appearance ?? "system",
      language: loaded.language ?? "system",
    };
    outputDir.value = output;
    await api.onFileUpdated(applyUpdate);
    await api.onBatchFinished(() => {
      running.value = false;
    });
    await api.onDragDrop((event) => {
      dragging.value = event.type === "over";
      if (event.type === "drop" && event.paths) void addPaths(event.paths);
      if (event.type === "leave") dragging.value = false;
    });
  }

  return {
    files,
    settings,
    outputDir,
    outputHint,
    outputError,
    selected,
    running,
    dragging,
    settingsOpen,
    aboutOpen,
    desktop,
    canStart,
    jpegAlpha,
    pngFromJpeg,
    showLossy,
    summary,
    addPaths,
    start,
    startOne: (id: string) => start(id),
    toggleSelected,
    toggleAllDownloadable,
    downloadOne,
    downloadZip,
    cancel,
    remove,
    clear,
    chooseOutput,
    openOutput,
    persist,
    init,
    pickFiles: async () => addPaths(await api.pickFiles()),
    pickFolder: async () => {
      const dir = await api.pickDirectory();
      if (dir) await addPaths([dir]);
    },
  };
});
