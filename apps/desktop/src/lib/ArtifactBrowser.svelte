<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import RevisionWorkspace from "./RevisionWorkspace.svelte";
  import StoryReader from "./StoryReader.svelte";
  import { completedAt, filterRuns } from "./artifactBrowserFilters";
  import type { RunSummary, WorkflowResult } from "./types";

  let { initialRunId = null }: { initialRunId?: string | null } = $props();
  let runs = $state<RunSummary[]>([]);
  let filteredRuns = $state<RunSummary[]>([]);
  let selected = $state<RunSummary | null>(null);
  let detail = $state<WorkflowResult | null>(null);
  let busy = $state(true);
  let error = $state("");
  let revising = $state(false);
  let reading = $state(false);
  let searchQuery = $state("");
  let sortBy = $state<"date" | "name">("date");
  let filterStatus = $state<"all" | "completed" | "failed">("all");
  let batchMode = $state(false);
  let selectedRunIds = $state<Set<string>>(new Set());
  let batchBusy = $state(false);
  let batchMessage = $state("");

  async function loadRuns() {
    busy = true;
    error = "";
    try {
      runs = await desktopApi.listRuns();
      applyFilters();
      const initial =
        filteredRuns.find((run) => run.run_id === initialRunId) ?? filteredRuns[0];
      if (initial) await selectRun(initial);
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  function applyFilters() {
    filteredRuns = filterRuns(runs, searchQuery, filterStatus, sortBy);
  }

  // Re-apply filters when search/filter/sort changes
  $effect(() => {
    searchQuery;
    sortBy;
    filterStatus;
    if (runs.length > 0) applyFilters();
  });

  async function selectRun(run: RunSummary) {
    selected = run;
    revising = false;
    reading = false;
    detail = null;
    try {
      detail = await desktopApi.readRun(run.run_id);
    } catch (value) {
      error = errorMessage(value);
    }
  }

  async function openRun(run: RunSummary) {
    if (selected?.run_id !== run.run_id || !detail) await selectRun(run);
    if (detail) reading = true;
  }

  function closeReader() {
    reading = false;
  }

  function toggleBatchMode() {
    batchMode = !batchMode;
    if (!batchMode) {
      selectedRunIds.clear();
      selectedRunIds = new Set();
    }
  }

  function toggleRunSelection(runId: string) {
    if (selectedRunIds.has(runId)) {
      selectedRunIds.delete(runId);
    } else {
      selectedRunIds.add(runId);
    }
    selectedRunIds = new Set(selectedRunIds);
  }

  function toggleSelectAll() {
    if (selectedRunIds.size === filteredRuns.length) {
      selectedRunIds.clear();
    } else {
      selectedRunIds = new Set(filteredRuns.map(run => run.run_id));
    }
    selectedRunIds = new Set(selectedRunIds);
  }

  async function batchExport(format: string) {
    if (selectedRunIds.size === 0) return;

    batchBusy = true;
    batchMessage = "";
    let successCount = 0;
    let failCount = 0;

    try {
      for (const runId of selectedRunIds) {
        try {
          const run = runs.find(r => r.run_id === runId);
          if (!run) continue;

          const workspace = await desktopApi.openRevisionWorkspace(runId);
          const approvedRevision = workspace.revisions.find(
            (rev) => rev.approval?.decision === "approved"
          );

          if (approvedRevision) {
            const timestamp = Date.now();
            const safeName = (run.logline || runId).replace(/[<>:"/\\|?*]/g, '_').slice(0, 50);
            const targetPath = `D:\\Stories\\batch_${timestamp}_${safeName}.${format}`;

            await desktopApi.exportRevision(approvedRevision.record.revision_id, targetPath);
            successCount++;
          } else {
            failCount++;
          }
        } catch {
          failCount++;
        }
      }

      batchMessage = `批量导出完成：成功 ${successCount}，失败 ${failCount}`;
      selectedRunIds.clear();
      selectedRunIds = new Set();
    } catch (value) {
      error = errorMessage(value);
    } finally {
      batchBusy = false;
    }
  }

  onMount(loadRuns);
</script>

<section class="panel artifact-panel">
  <div class="panel-heading">
    <div>
      <span class="eyebrow">作品库</span>
      <h2>已完成的故事包</h2>
    </div>
    <div class="panel-actions">
      <button class="ghost" onclick={toggleBatchMode} disabled={busy || !runs.length}>
        {batchMode ? "退出批量" : "批量操作"}
      </button>
      <button class="ghost" onclick={loadRuns} disabled={busy}>刷新</button>
    </div>
  </div>

  <div class="search-filters">
    <input
      type="text"
      bind:value={searchQuery}
      placeholder="搜索标题、运行ID、模型..."
      class="search-input"
    />
    <select bind:value={sortBy} class="sort-select">
      <option value="date">按日期排序</option>
      <option value="name">按名称排序</option>
    </select>
    <select bind:value={filterStatus} class="filter-select">
      <option value="all">全部状态</option>
      <option value="completed">已完成</option>
      <option value="failed">未完成</option>
    </select>
    <span class="result-count">{filteredRuns.length} / {runs.length} 个故事</span>
  </div>

  {#if batchMode}
    <div class="batch-toolbar">
      <button class="ghost" onclick={toggleSelectAll}>
        {selectedRunIds.size === filteredRuns.length ? "取消全选" : "全选"}
      </button>
      <span class="batch-count">已选择 {selectedRunIds.size} 个故事</span>
      <div class="batch-actions">
        <button
          class="secondary"
          onclick={() => batchExport("json")}
          disabled={batchBusy || selectedRunIds.size === 0}
        >
          批量导出 JSON
        </button>
        <button
          class="secondary"
          onclick={() => batchExport("md")}
          disabled={batchBusy || selectedRunIds.size === 0}
        >
          批量导出 Markdown
        </button>
      </div>
      {#if batchMessage}<p class="success">{batchMessage}</p>{/if}
    </div>
  {/if}

  {#if busy && !runs.length}
    <div class="empty-state">正在读取本地作品库…</div>
  {:else if error && !runs.length}
    <div class="empty-state error">{error}</div>
  {:else if !runs.length}
    <div class="empty-state">还没有完成的 advisory 故事包。</div>
  {:else if !filteredRuns.length}
    <div class="empty-state">没有找到匹配的故事。</div>
  {:else}
    <div class="artifact-layout">
      <div class="run-list">
        {#each filteredRuns as run, index}
          <button
            class:active={selected?.run_id === run.run_id}
            class="run-card"
            onclick={() => selectRun(run)}
            ondblclick={() => openRun(run)}
            title="单击选择，双击查看完整故事"
          >
            <span>
              {run.generation_model} → {run.review_model}
              {#if index === 0}<b class="latest-badge">最新生成</b>{/if}
            </span>
            <strong>{run.logline || `${run.episode_count} 集故事包`}</strong>
            <small>{completedAt(run.completed_at_unix_ms)} · {run.run_id.slice(-8)}</small>
          </button>
        {/each}
      </div>

      <div class="artifact-detail">
        {#if revising && selected}
          <RevisionWorkspace runId={selected.run_id} onclose={() => (revising = false)} />
        {:else}
        {#if detail && selected}
          <div class="artifact-meta">
            <span class="status-chip">ADVISORY</span>
            <span>{selected.task_count}/17 TASKS</span>
            <span>{selected.review_count} REVIEWS</span>
          </div>
          <h3>{detail.package.logline?.text ?? "未提供故事梗概"}</h3>
          <div class="metric-row">
            <div><strong>{detail.package.characters?.length ?? 0}</strong><span>角色</span></div>
            <div><strong>{detail.package.episodes?.length ?? 0}</strong><span>分集</span></div>
            <div><strong>{detail.package.scenes?.length ?? 0}</strong><span>代表场景</span></div>
          </div>
          <div class="review-strip">
            {#each detail.reviews as review}
              <div class="review-item">
                <span>{review.review_type}</span>
                <strong>{review.status}</strong>
                <small>{review.findings.length} findings</small>
              </div>
            {/each}
          </div>
          <div class="action-row">
            <button class="primary" onclick={() => (reading = true)}>
              查看完整故事
            </button>
            <button class="primary" onclick={() => (revising = true)}>
              打开修订工作区
            </button>
          </div>
        {:else}
          <div class="empty-state">选择一个故事包查看详情。</div>
        {/if}
        {/if}
      </div>
    </div>
  {/if}
</section>

{#if reading && detail && selected}
  <StoryReader result={detail} run={selected} onclose={closeReader} />
{/if}
