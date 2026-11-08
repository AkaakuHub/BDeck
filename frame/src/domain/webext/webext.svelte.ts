import { commands, type WebExtensionReview, type WebExtensionView, type WebExtensionAccessRequestView } from "$shared/ipc/bindings";
import { events } from "$shared/ipc/native-events";

let installed = $state<WebExtensionView[]>([]);
let review = $state<WebExtensionReview | null>(null);
let error = $state<string | null>(null);
let busy = $state(false);
let preparing = $state(false);
let preparingExtension = $state<string | null>(null);
let requests = $state<WebExtensionAccessRequestView[]>([]);
let refreshTimer: ReturnType<typeof setTimeout> | undefined;
let generation = 0;
let stop: (() => void) | null = null;
export const list = () => installed;
export const pending = () => review;
export const failure = () => error;
export const working = () => busy;
export const isPreparing = () => preparing;
export const preparingId = () => preparingExtension;
export const accessRequest = () => requests[0] ?? null;

export async function refresh() {
  const current = generation;
  try {
    const result = await commands.webExtensionList();
    if (current !== generation) return;
    installed = result;
    error = null;
    clearTimeout(refreshTimer);
    if (result.some(extension => extension.state === "starting")) refreshTimer = setTimeout(() => void refresh(), 600);
  } catch (reason) { if (current === generation) error = String(reason); }
}
export async function retain(onAccess: () => void) {
  const current = ++generation;
  const unlisten = await events.webExtensionAccessRequested.listen(({ payload }) => { requests = [...requests, payload]; onAccess(); });
  if (current !== generation) { unlisten(); return; }
  stop = unlisten;
  await refresh();
}
export function release() {
  generation += 1;
  clearTimeout(refreshTimer);
  stop?.(); stop = null;
  if (review) void commands.webExtensionCancel(review.review_id);
  review = null; installed = []; requests = []; busy = false; preparing = false; preparingExtension = null; error = null;
}
async function perform(action: () => Promise<void>) {
  if (busy) return;
  const current = generation;
  busy = true; error = null;
  try { await action(); }
  catch (reason) { if (current === generation) error = String(reason); }
  finally { if (current === generation) busy = false; }
}
async function prepare(request: () => Promise<WebExtensionReview | null>, extension: string | null = null) {
  await perform(async () => {
    const current = generation;
    preparing = true; preparingExtension = extension;
    try {
      const result = await request();
      if (current !== generation) { if (result) await commands.webExtensionCancel(result.review_id); return; }
      if (review) await commands.webExtensionCancel(review.review_id);
      review = result;
    } finally {
      if (current === generation) { preparing = false; preparingExtension = null; }
    }
  });
}
export const prepareListing = (url: string) => prepare(() => commands.webExtensionPrepare(url));
export const chooseFile = (folder: boolean) => prepare(() => commands.webExtensionChooseFile(folder));
export const reviewUpdate = (id: string) => prepare(() => commands.webExtensionReviewUpdate(id), id);
export const confirm = () => perform(async () => {
  const current = review;
  if (!current) return;
  await commands.webExtensionConfirm(current.id, current.review_id);
  if (review?.review_id === current.review_id) review = null;
  await refresh();
});
export const cancel = () => perform(async () => {
  const current = review;
  if (!current) return;
  await commands.webExtensionCancel(current.review_id);
  if (review?.review_id === current.review_id) review = null;
});
export const setEnabled = (id: string, enabled: boolean) => perform(async () => { await commands.webExtensionSetEnabled(id, enabled); await refresh(); });
export const uninstall = (id: string) => perform(async () => { await commands.webExtensionUninstall(id); await refresh(); });
export const retry = (extension: WebExtensionView) => perform(async () => { await commands.webExtensionRetry(extension.install_id); await refresh(); });
export const openOptions = (id: string) => perform(async () => { await commands.webExtensionOpenOptions(id); });
export const setAccess = (id: string, mode: string, sites: string[]) => perform(async () => { await commands.webExtensionSetAccess(id, mode, sites); await refresh(); });
export const answerAccess = (allowed: boolean) => perform(async () => {
  const request = requests[0];
  if (!request) return;
  await commands.webExtensionAnswerAccess(request, allowed);
  requests = requests.slice(1);
});
