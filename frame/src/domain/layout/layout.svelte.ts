import type { LayoutState } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";
let state = $state<LayoutState>({ dividers: [], columns: [], composer: null, scroll: null });
let stop: (() => void) | null = null;
async function init() {
  if (stop) return;
  stop = await events.layoutChanged.listen(({ payload }) => { state = payload; });
}
function dispose() { stop?.(); stop = null; state = { dividers: [], columns: [], composer: null, scroll: null }; }
export const layout = { init, dispose, columns: () => state.columns, composer: () => state.composer, scroll: () => state.scroll };
