import type { UpdateStatus, ExtensionActionsView, ExtensionActionFailedView, ExtensionActionShortcutView, WebExtensionAccessRequestView, OperationDisposition, PanelSnapshot, PanelAction, ItemsState, LayoutState, ProfilesView, TabView } from "./bindings";

type PayloadEvent<T> = { payload: T };
type Listener<T> = (event: PayloadEvent<T>) => void;

function scopedEvent<T>(name: string) {
  return {
    listen(listener: Listener<T>): Promise<() => void> {
      const handler = (event: Event) => listener({ payload: (event as CustomEvent<T>).detail });
      window.addEventListener(name, handler);
      return Promise.resolve(() => window.removeEventListener(name, handler));
    },
  };
}

export const events = {
  updater: scopedEvent<UpdateStatus>("bdeck:updater"),
  extensionActionsChanged: scopedEvent<ExtensionActionsView>("zephium:extension-actions"),
  extensionActionFailed: scopedEvent<ExtensionActionFailedView>("zephium:extension-action-failed"),
  extensionActionShortcut: scopedEvent<ExtensionActionShortcutView>("zephium:extension-action-shortcut"),
  webExtensionAccessRequested: scopedEvent<WebExtensionAccessRequestView>("zephium:web-extension-access"),
  operationProcessed: scopedEvent<OperationDisposition>("zephium:operation-processed"),
  columnPanelAction: scopedEvent<[string | null, PanelAction]>("zephium:column-panel-action"),
  columnPanelOpen: scopedEvent<PanelSnapshot | null>("zephium:column-panel-open"),
  columnPanelView: scopedEvent<PanelSnapshot | null>("zephium:column-panel-view"),
  layoutChanged: scopedEvent<LayoutState>("zephium:layout"),
  uiCommand: scopedEvent<string>("zephium:ui-command"),
  presentationTab: scopedEvent<{ tab: TabView; active: string | null }>("zephium:presentation-tab"),
  browserReturn: scopedEvent<ItemsState>("zephium:browser-return"),
  itemsChanged: scopedEvent<ItemsState>("zephium:items"),
  tabChanged: scopedEvent<TabView>("zephium:tab"),
  profilesChanged: scopedEvent<ProfilesView>("zephium:profiles"),
};
