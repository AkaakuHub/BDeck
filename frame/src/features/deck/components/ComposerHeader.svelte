<script lang="ts">
  import { layout } from "$domain/layout";
  import { accountAvatars, deck } from "$domain/deck";
  import { commands, type PanelView } from "$shared/ipc/bindings";
  import AccountPanel from "./AccountPanel.svelte";
  import { onDestroy } from "svelte";
  const frame = $derived(layout.composer());
  let signature = "";
  let queued: { view: PanelView | null; x: number; y: number } | null = null;
  let updating = false;
  async function update() {
    updating = true;
    while (queued) {
      const current = queued;
      queued = null;
      try { await commands.columnPanelComposerSync(current.view, current.x, current.y); }
      catch { console.error("composer surface update failed"); }
    }
    updating = false;
  }
  onDestroy(() => { queued = null; });
  $effect(() => {
    const candidate = frame;
    const current = candidate && typeof candidate.x === "number" && typeof candidate.y === "number" && typeof candidate.width === "number" && typeof candidate.height === "number"
      ? { ...candidate, x: candidate.x, y: candidate.y, width: candidate.width, height: candidate.height } : null;
    const view: PanelView | null = current && !current.managing ? {
      composer: true, content_height: Math.round(current.height - 42), id: current.tab, title: "", index: 0, count: 0,
      account: current.account,
      accounts: deck.readyAccounts().map(account => ({ id: account.id, title: deck.displayName(account.id), screen_name: account.identity?.screen_name ?? null, avatar: accountAvatars.url(account.profile) })),
      width: Math.round(current.width), row: 0, two_rows: false, suspended: false, secret: false, failed: false,
    } : null;
    const x = current ? current.x + current.width : 0;
    const y = current?.y ?? 0;
    const next = JSON.stringify([view, x, y]);
    if (next === signature) return;
    signature = next;
    queued = { view, x, y };
    if (!updating) void update();
  });
</script>
{#if frame?.managing}<AccountPanel {frame} />{/if}
