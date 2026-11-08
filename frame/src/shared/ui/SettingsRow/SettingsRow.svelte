<script lang="ts">
  import type { Snippet } from "svelte";
  let {
    title,
    children,
    settingId,
  }: { title: string; children: Snippet; settingId?: string } = $props();
</script>

<div class="ui-settings-row" data-setting={settingId}>
  <div class="ui-settings-copy">
    <h3>{title}</h3>
  </div>
  <div class="ui-settings-control">{@render children()}</div>
</div>

<style>
  .ui-settings-row {
    position: relative;
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 24px;
    box-sizing: border-box;
    min-height: var(--row-page);
    padding: 14px 18px;
  }

  :global(.ui-settings-row) + .ui-settings-row::before {
    content: "";
    position: absolute;
    inset-inline: 16px 0;
    top: 0;
    height: 1px;
    background: var(--color-border);
  }

  .ui-settings-copy {
    min-width: 0;
  }

  h3 {
    margin: 0;
    color: var(--color-text);
    font-size: var(--text-page-title);
    font-weight: 500;
    line-height: 19px;
    letter-spacing: -0.008em;
  }


  .ui-settings-control {
    flex: none;
    max-width: 58%;
  }
</style>
