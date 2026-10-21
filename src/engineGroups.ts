import { computed } from "vue";
import { t } from "./i18n";
import type { Settings } from "./types";

export type EngineKey = keyof Settings["engines"];

export function useEngineGroups() {
  return computed<Array<{ title: string; items: Array<[EngineKey, string, string]> }>>(() => [
    {
      title: "PNG",
      items: [
        ["oxipng", t("engineOxipng"), t("engineOxipngHint")],
        ["pngquant", t("enginePngquant"), t("enginePngquantHint")],
        ["zopflipng", t("engineZopfli"), t("engineZopfliHint")],
      ],
    },
    {
      title: "JPEG",
      items: [
        ["jpegtran", t("engineJpegtran"), t("engineJpegtranHint")],
        ["jpegoptim", t("engineJpegoptim"), t("engineJpegoptimHint")],
      ],
    },
    {
      title: "GIF",
      items: [["gifsicle", t("engineGifsicle"), t("engineGifsicleHint")]],
    },
    {
      title: "SVG",
      items: [["svgcleaner", t("engineSvgcleaner"), t("engineSvgcleanerHint")]],
    },
  ]);
}
