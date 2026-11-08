import type { OperationAdmission, OperationDisposition } from "./bindings";
import { events } from "./native-events";

type Result = { kind: "cancelled" | "rejected" } | { kind: "processed"; disposition: OperationDisposition };

export async function runOperation(invoke: () => Promise<OperationAdmission | null>): Promise<Result> {
  let id: string | null = null;
  const early = new Map<string, OperationDisposition>();
  let settle: (result: OperationDisposition) => void = () => {};
  const completion = new Promise<OperationDisposition>((resolve) => { settle = resolve; });
  const stop = await events.operationProcessed.listen(({ payload }) => {
    if (id === payload.operation_id) settle(payload);
    else if (id === null) {
      early.set(payload.operation_id, payload);
      if (early.size > 64) early.delete(early.keys().next().value ?? "");
    }
  });
  try {
    const admission = await invoke();
    if (!admission) return { kind: "cancelled" };
    if (!admission.accepted || !admission.operation_id) return { kind: "rejected" };
    id = admission.operation_id;
    const processed = early.get(id);
    early.clear();
    return { kind: "processed", disposition: processed ?? await completion };
  } finally { stop(); }
}

export function accepted(result: Result): boolean {
  if (result.kind !== "processed") return false;
  const { outcome, reason } = result.disposition;
  return outcome === "applied" || outcome === "no_op" || (outcome === "deferred" && (reason === "store_pending" || reason === "native_pending"));
}
