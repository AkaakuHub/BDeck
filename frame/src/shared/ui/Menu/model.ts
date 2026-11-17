import type { IconSvgElement } from "@hugeicons/svelte";

export type MenuEntry =
  | {
      kind: "item";
      id: string;
      label: string;
      icon?: IconSvgElement;
      checked?: boolean;
      danger?: boolean;
      disabled?: boolean;
    }
  | { kind: "separator" }
  | { kind: "heading"; label: string };
