<script lang="ts">
  import { onMount } from "svelte";
  import "$styles/global.css";
  import { commands } from "$shared/ipc/bindings";
  import { tabs } from "$domain/tabs";
  import { surface } from "$domain/surface";
  import { columnPanel } from "$domain/deck";
  import { deck } from "$domain/deck";
  import { extensions } from "$domain/extensions";
  import { webext } from "$domain/webext";
  import { layout } from "$domain/layout";
  import { updater } from "$domain/updater";
  import Shell from "./Shell.svelte";

  onMount(() => {
    let disposed = false;
    let stopPanel: (() => void) | undefined;
    const shortcut = (event: KeyboardEvent) => {
      if (event.defaultPrevented || event.isComposing || event.metaKey || event.ctrlKey || event.altKey || event.shiftKey || surface.visible()) return;
      if (event.target instanceof Element && event.target.closest("input, textarea, select, [contenteditable], [role=textbox]")) return;
      if (event.key === "n") { event.preventDefault(); void deck.openComposer(); }
      if (event.key === "Escape" && layout.composer()) { event.preventDefault(); void commands.runCommand("composer.close"); }
    };
    const horizontalScroll = (event: WheelEvent) => {
      const frame = layout.scroll();
      if (!frame || surface.visible() || !event.isTrusted || event.ctrlKey || event.metaKey || !(event.target instanceof Element) || !event.target.closest(".column-header")) return;
      const delta = event.shiftKey ? event.deltaX || event.deltaY : Math.abs(event.deltaX) > Math.abs(event.deltaY) ? event.deltaX : 0;
      if (!Number.isFinite(delta) || Math.abs(delta) < 0.5) return;
      event.preventDefault();
      const scale = event.deltaMode === 1 ? 16 : event.deltaMode === 2 ? frame.width ?? innerWidth : 1;
      const offset = Math.max(0, Math.min(frame.max_offset ?? 0, (frame.offset ?? 0) + delta * scale));
      void commands.runCommand(`deck.scroll=${offset}`).catch(() => console.error("column scroll failed"));
    };
    window.addEventListener("wheel", horizontalScroll, { passive: false });
    window.addEventListener("keydown", shortcut);
    const tabsReady = surface.init().then(async () => {
      await webext.retain(() => { void surface.open("extensions"); });
      if (disposed) { webext.release(); return; }
      await extensions.init();
      await tabs.init();
      if (disposed) return;
      await Promise.all([deck.init(), layout.init(), updater.init()]);
    });

    void (async () => {
      try {
        await tabsReady;
        stopPanel = await columnPanel.init();
        if (disposed) { stopPanel(); return; }
        if (!(await commands.uiReady())) return;
      } catch {
        console.error("trusted browser client initialization failed");
      }
    })();

    return () => {
      disposed = true;
      window.removeEventListener("keydown", shortcut);
      window.removeEventListener("wheel", horizontalScroll);
      webext.release();
      extensions.dispose();
      tabs.dispose();
      surface.dispose();
      stopPanel?.();
      deck.dispose();
      layout.dispose();
      updater.dispose();
    };
  });
</script>

<Shell />
