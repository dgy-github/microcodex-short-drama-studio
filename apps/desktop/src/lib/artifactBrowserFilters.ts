import type { RunSummary } from "./types";

export type RunStatusFilter = "all" | "completed" | "failed";
export type RunSort = "date" | "name";

export function filterRuns(
  runs: RunSummary[], query: string, status: RunStatusFilter, sort: RunSort,
) {
  let result = [...runs];
  if (query.trim()) {
    const normalized = query.toLowerCase();
    result = result.filter((run) =>
      (run.logline || "").toLowerCase().includes(normalized)
      || run.run_id.toLowerCase().includes(normalized)
      || `${run.generation_model} ${run.review_model}`.toLowerCase().includes(normalized));
  }
  if (status !== "all") {
    result = result.filter((run) => status === "completed" ? run.task_count >= 17 : run.task_count < 17);
  }
  result.sort(sort === "date"
    ? (left, right) => right.completed_at_unix_ms - left.completed_at_unix_ms
    : (left, right) => (left.logline || "").localeCompare(right.logline || ""));
  return result;
}

export function completedAt(timestamp: number) {
  return new Date(timestamp).toLocaleString("zh-CN", { hour12: false });
}
