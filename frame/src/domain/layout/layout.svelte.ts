import type { LayoutState } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";
let state = $state<LayoutState>({
  dividers: [],
  columns: [],
  concealed_columns: [],
  composer: null,
  scroll: null,
});
let stop: (() => void) | null = null;
async function init() {
  if (stop) return;
  stop = await events.layoutChanged.listen(({ payload }) => {
    state = payload;
  });
}
function dispose() {
  stop?.();
  stop = null;
  state = { dividers: [], columns: [], concealed_columns: [], composer: null, scroll: null };
}
export const layout = {
  init,
  dispose,
  columns: () => state.columns,
  concealed: () => state.concealed_columns,
  composer: () => state.composer,
  scroll: () => state.scroll,
};
