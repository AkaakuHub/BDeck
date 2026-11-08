import type { DeckAppearance } from "$shared/ipc/bindings";
import { isRecord } from "./deck-types";

export const DEFAULT_APPEARANCE: DeckAppearance = {
  font_size: 14, density: "comfortable", avatars: "circle",
  media: "native", scrollbars: "thin", hide_navigation: true,
};

export function isDeckAppearance(value: unknown): value is DeckAppearance {
  return isRecord(value)
    && typeof value.font_size === "number" && Number.isInteger(value.font_size) && value.font_size >= 12 && value.font_size <= 18
    && (value.density === "comfortable" || value.density === "compact")
    && (value.avatars === "circle" || value.avatars === "square")
    && (value.media === "native" || value.media === "compact" || value.media === "hidden")
    && (value.scrollbars === "system" || value.scrollbars === "thin" || value.scrollbars === "hidden")
    && typeof value.hide_navigation === "boolean";
}
