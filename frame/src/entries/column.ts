import { getCurrentWindow } from "@tauri-apps/api/window";
import { mount, flushSync } from "svelte";
import ColumnPanel from "$features/deck/column";

if (getCurrentWindow().label !== "column-panel") throw new Error("unexpected native surface");
const target = document.getElementById("root");
if (!(target instanceof HTMLElement)) throw new Error("trusted UI root is unavailable");
mount(ColumnPanel, { target });
flushSync();
