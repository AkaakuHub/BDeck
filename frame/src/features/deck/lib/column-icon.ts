import { Home01Icon, Notification01Icon, Mail01Icon, FireIcon, Search01Icon, Layout01Icon, UserCircleIcon } from "@hugeicons/core-free-icons";
import type { ColumnDraft } from "$shared/ipc/bindings";
import type { ColumnTarget } from "$domain/deck";

export function columnIcon(target: ColumnTarget | Pick<ColumnDraft, "kind">) {
  if (target.kind === "home") return Home01Icon;
  if (target.kind === "notifications") return Notification01Icon;
  if (target.kind === "profile") return UserCircleIcon;
  if (target.kind === "search") return Search01Icon;
  if (target.kind === "messages" || (target.kind === "url" && target.url === "https://x.com/messages")) return Mail01Icon;
  if (target.kind === "explore" || (target.kind === "url" && target.url === "https://x.com/explore")) return FireIcon;
  return Layout01Icon;
}
