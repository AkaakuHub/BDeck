import type { IconSvgElement } from "@hugeicons/svelte";

export type SegmentedOption = {
  value: string;
  label: string;
  icon?: IconSvgElement;
  disabled?: boolean;
};
