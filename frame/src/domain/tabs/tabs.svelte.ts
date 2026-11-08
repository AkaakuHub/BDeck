import { flushSync } from "svelte";
import type { ItemsState, TabView } from "$shared/ipc/bindings";
import { commands } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";
import { TabProjectionModel } from "./tabs-model";

const model = new TabProjectionModel();
let state = $state.raw<ItemsState>(model.value);
let started = false;
let stops: (() => void)[] = [];

export const tabs = () => state.tabs;
export const activeId = () => state.active;

function publish() {
  state = model.value;
}

export async function init() {
  if (started) return;
  const [stopItems, stopTab, stopPresentation, stopReturn] = await Promise.all([
    events.itemsChanged.listen(({ payload }) => {
      if (model.applySnapshot(payload)) publish();
    }),
    events.tabChanged.listen(({ payload }) => {
      if (model.applyTab(payload)) publish();
    }),
    events.presentationTab.listen(({ payload }) => {
      flushSync(() => {
        if (model.applyPresentation(payload)) publish();
      });
    }),
    events.browserReturn.listen(({ payload }) => {
      flushSync(() => {
        if (model.applySnapshot(payload)) publish();
      });
    }),
  ]);
  stops = [stopItems, stopTab, stopPresentation, stopReturn];
  started = true;
  await commands.tabsBootstrap();
}

export function dispose() {
  for (const stop of stops) stop();
  stops = [];
  started = false;
}

export const activate = (id: string) => void commands.tabsActivate(id);

export type { TabView };
