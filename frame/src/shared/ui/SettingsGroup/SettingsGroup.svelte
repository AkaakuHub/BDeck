<script lang="ts">
  import type { Snippet } from "svelte";
  let { title, children }: { title: string; children: Snippet } =
    $props();
  const uid = $props.id();
</script>

<section class="ui-settings-section" aria-labelledby={uid}>
  <h2 id={uid}>{title}</h2>
  <div class="ui-settings-group">{@render children()}</div>
</section>

<style>
  .ui-settings-section {
    margin-block: 0 34px;
  }

  h2 {
    margin: 0 0 10px;
    padding-inline: 16px;
    color: var(--color-text);
    font-size: 15px;
    font-weight: 600;
    line-height: 20px;
    letter-spacing: -0.012em;
  }

  .ui-settings-group {
    overflow: hidden;
    border-radius: var(--radius-card);
    background: var(--color-surface);
    box-shadow: var(--shadow-card);
  }


  @media (forced-colors: active) {
    .ui-settings-group {
      border: 1px solid ButtonText;
      box-shadow: none;
    }
  }
</style>
