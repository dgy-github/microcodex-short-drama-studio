<script lang="ts">
  import type { RunSnapshot } from "./types";

  let {
    snapshot,
    cancelling,
    oncancel,
  }: {
    snapshot: RunSnapshot;
    cancelling: boolean;
    oncancel: () => void;
  } = $props();

  const terminal = $derived(
    ["completed", "failed", "cancelled"].includes(snapshot.status),
  );
  const progress = $derived(
    Math.round((snapshot.tasks_completed / snapshot.tasks_total) * 100),
  );
</script>

<section class="run-console" aria-live="polite">
  <div class="run-head">
    <div>
      <span class="eyebrow">LIVE RUN · {snapshot.run_id.slice(-8)}</span>
      <h3>{snapshot.status.toUpperCase()}</h3>
    </div>
    <button
      class="ghost danger"
      onclick={oncancel}
      disabled={terminal || cancelling}
    >
      {cancelling ? "正在取消…" : "中止运行"}
    </button>
  </div>

  <div class="progress-track">
    <span style={`width: ${progress}%`}></span>
  </div>
  <div class="run-metrics">
    <div><strong>{snapshot.tasks_completed}/17</strong><span>已完成任务</span></div>
    <div><strong>{snapshot.reviews_completed}/5</strong><span>审查记录</span></div>
    <div><strong>{snapshot.approvals_pending}</strong><span>待审批</span></div>
    <div>
      <strong>{snapshot.budget.consumed_tokens.toLocaleString()}</strong>
      <span>Token / {snapshot.budget.max_tokens.toLocaleString()}</span>
    </div>
  </div>

  {#if snapshot.budget.consumed_cny_fen === null}
    <p class="budget-note">
      费用消耗暂不可计算；上限为 ¥{(snapshot.budget.max_cny_fen / 100).toFixed(2)}。
    </p>
  {/if}
  {#if snapshot.error}<p class="error">{snapshot.error}</p>{/if}

  <div class="event-feed">
    {#if snapshot.events.length === 0}
      <span>等待新的持久事件… Last-Event-ID: {snapshot.last_event_id}</span>
    {:else}
      {#each [...snapshot.events].reverse() as event (event.event_id)}
        <div>
          <code>#{event.seq}</code>
          <strong>{event.event_type}</strong>
          <span>{event.task_id ?? "run"}</span>
        </div>
      {/each}
    {/if}
  </div>
</section>
