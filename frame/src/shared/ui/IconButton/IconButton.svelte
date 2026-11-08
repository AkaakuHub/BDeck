<script lang="ts">
  import type { IconSvgElement } from "@hugeicons/svelte";
  import Icon from "../Icon/Icon.svelte";
  let { icon, label, size = 16, buttonSize = 28, active = false, disabled = false, onclick }: {
    icon: IconSvgElement;
    label: string;
    size?: number;
    buttonSize?: number;
    active?: boolean;
    disabled?: boolean;
    onclick?: (event: MouseEvent) => void;
  } = $props();
</script>

<button type="button" class="icon-button" class:icon-button-active={active} aria-label={label} title={label} aria-pressed={active || undefined} {disabled} {onclick} style:--icon-button-size={`${buttonSize}px`}>
  <Icon {icon} {size} />
</button>
