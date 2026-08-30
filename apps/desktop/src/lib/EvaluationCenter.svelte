<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import BlindReviewPanel from "./BlindReviewPanel.svelte";
  import EvaluationCaseDetail from "./EvaluationCaseDetail.svelte";
  import type {
    BlindAssignment,
    EvaluationBatchResult,
    EvaluationCatalog,
    EvaluationCase,
    EvaluationDataset,
    EvaluationScoreRecord,
    HumanDimensionInput,
  } from "./types";

  let catalog = $state<EvaluationCatalog | null>(null);
  let datasetId = $state<EvaluationDataset["dataset_id"]>("offline-v0.1.0");
  let selected = $state<Set<string>>(new Set());
  let mode = $state<"automatic" | "human">("automatic");
  let raterId = $state("reviewer_01");
  let busy = $state(false);
  let error = $state("");
  let batch = $state<EvaluationBatchResult | null>(null);
  let assignments = $state<BlindAssignment[]>([]);
  let assignmentIndex = $state(0);
  let scores = $state<HumanDimensionInput[]>([]);
  let humanResults = $state<EvaluationScoreRecord[]>([]);
  let detailCase = $state<EvaluationCase | null>(null);

  const activeDataset = $derived(
    catalog?.datasets.find((dataset) => dataset.dataset_id === datasetId) ?? null,
  );
  const activeAssignment = $derived(assignments[assignmentIndex] ?? null);

  onMount(loadCatalog);

  async function loadCatalog() {
    busy = true;
    error = "";
    try {
      catalog = await desktopApi.evaluationCatalog();
      if (!catalog.datasets.some((dataset) => dataset.dataset_id === datasetId)) {
        datasetId = catalog.datasets[0].dataset_id;
      }
      selected = new Set();
    } catch (reason) {
      error = errorMessage(reason);
    } finally {
      busy = false;
    }
  }

  function chooseDataset(id: EvaluationDataset["dataset_id"]) {
    datasetId = id;
    selected = new Set();
    batch = null;
    detailCase = null;
  }

  function openCaseDetail(item: EvaluationCase) {
    detailCase = item;
  }

  function closeCaseDetail() {
    detailCase = null;
  }

  function handleCaseDoubleClick(event: MouseEvent, item: EvaluationCase) {
    const target = event.target as HTMLElement;
    if (target.closest("input, button")) return;
    openCaseDetail(item);
  }

  function handleGlobalKeydown(event: KeyboardEvent) {
    if (event.key === "Escape" && detailCase) closeCaseDetail();
  }

  function toggle(caseId: string) {
    const next = new Set(selected);
    if (next.has(caseId)) next.delete(caseId);
    else next.add(caseId);
    selected = next;
  }

  function eligibleIds() {
    return activeDataset?.cases.filter((item) => item.eligible).map((item) => item.case_id) ?? [];
  }

  function selectAllEligible() {
    selected = new Set(eligibleIds());
  }

  async function runAutomatic(caseIds = [...selected]) {
    if (!caseIds.length) return;
    busy = true;
    error = "";
    batch = null;
    try {
      batch = await desktopApi.runAutomaticEvaluation(datasetId, caseIds);
    } catch (reason) {
      error = errorMessage(reason);
    } finally {
      busy = false;
    }
  }

  async function createBlind(caseIds = [...selected]) {
    if (!caseIds.length || !raterId.trim()) return;
    busy = true;
    error = "";
    try {
      assignments = await desktopApi.createBlindAssignments(
        datasetId,
        caseIds,
        raterId.trim(),
      );
      assignmentIndex = 0;
      humanResults = [];
      prepareScores();
    } catch (reason) {
      error = errorMessage(reason);
    } finally {
      busy = false;
    }
  }

  function prepareScores() {
    if (!activeAssignment) {
      scores = [];
      return;
    }
    scores = activeAssignment.dimensions.map((dimension) => ({
      dimension_id: dimension.dimension_id,
      score: 3,
      reason: "",
      span_refs: [activeAssignment.allowed_spans[0] ?? ""],
    }));
  }

  function updateScore(index: number, key: "score" | "reason" | "span", value: string) {
    scores = scores.map((item, itemIndex) => {
      if (itemIndex !== index) return item;
      if (key === "score") return { ...item, score: Number(value) };
      if (key === "reason") return { ...item, reason: value };
      return { ...item, span_refs: [value] };
    });
  }

  async function submitBlind() {
    if (!activeAssignment) return;
    busy = true;
    error = "";
    try {
      const result = await desktopApi.submitBlindReview(
        activeAssignment.assignment_id,
        raterId.trim(),
        scores,
      );
      humanResults = [...humanResults, result];
      assignmentIndex += 1;
      prepareScores();
    } catch (reason) {
      error = errorMessage(reason);
    } finally {
      busy = false;
    }
  }

  function leaveBlind() {
    assignments = [];
    assignmentIndex = 0;
    scores = [];
  }
</script>

<svelte:window onkeydown={handleGlobalKeydown} />

{#if activeAssignment}
  <BlindReviewPanel assignment={activeAssignment} {assignmentIndex}
    assignmentCount={assignments.length} {scores} {busy} {error}
    onleave={leaveBlind} onupdate={updateScore} onsubmit={submitBlind} />
{:else}
  <section class="panel evaluation-panel">
    <div class="panel-heading">
      <div>
        <span class="eyebrow">EVALUATION CONTROL</span>
        <h2>评测中心</h2>
      </div>
      <button class="ghost" onclick={loadCatalog} disabled={busy}>刷新目录</button>
    </div>

    {#if catalog}
      <div class="dataset-switch">
        {#each catalog.datasets as dataset}
          <button
            class:active={dataset.dataset_id === datasetId}
            onclick={() => chooseDataset(dataset.dataset_id)}
          >
            <strong>{dataset.label}</strong>
            <span>{dataset.eligible_count}/{dataset.case_count} 可运行</span>
          </button>
        {/each}
      </div>

      <div class="evaluation-toolbar">
        <div class="mode-switch">
          <button class:active={mode === "automatic"} onclick={() => (mode = "automatic")}>
            自动评测
          </button>
          <button class:active={mode === "human"} onclick={() => (mode = "human")}>
            人工盲测
          </button>
        </div>
        {#if mode === "human"}
          <label class="compact-field">评审编号
            <input bind:value={raterId} maxlength="64" />
          </label>
        {/if}
        <button class="ghost" onclick={selectAllEligible}>选择全部可运行</button>
        <span>{selected.size} 项已选择</span>
      </div>

      <div class="evaluation-cases">
        {#each activeDataset?.cases ?? [] as item}
          <div
            class="case-row"
            class:disabled={!item.eligible}
            role="group"
            aria-label={`评测用例 ${item.case_id}`}
            ondblclick={(event) => handleCaseDoubleClick(event, item)}
            title="双击查看用例详情"
          >
            <input
              type="checkbox"
              aria-label={`选择用例 ${item.case_id}`}
              checked={selected.has(item.case_id)}
              disabled={!item.eligible || busy}
              onchange={() => toggle(item.case_id)}
            />
            <span class="case-id">{item.case_id}</span>
            <span class="case-copy"><strong>{item.label}</strong><small>{item.genre} · {item.difficulty ?? "真实运行"} · {item.split ?? "online"}</small></span>
            <span class:ready={item.eligible} class="eligibility">{item.eligible ? "READY" : "NO ARTIFACT"}</span>
            <button class="case-detail-button" type="button" onclick={() => openCaseDetail(item)}>
              详情
            </button>
          </div>
        {/each}
      </div>
      <p class="case-hint">双击任意用例行，或点击“详情”查看完整明细。</p>

      <div class="action-row">
        {#if mode === "automatic"}
          <button class="primary" onclick={() => runAutomatic()} disabled={busy || !selected.size}>
            {busy ? "评测中…" : "运行所选自动评测"}
          </button>
          <button class="secondary" onclick={() => runAutomatic(eligibleIds())} disabled={busy || !activeDataset?.eligible_count}>
            批量运行全部
          </button>
          <span class="shield">单 Qwen judge · advisory · 不用于晋升</span>
        {:else}
          <button class="primary" onclick={() => createBlind()} disabled={busy || !selected.size || !raterId.trim()}>
            创建所选盲测
          </button>
          <button class="secondary" onclick={() => createBlind(eligibleIds())} disabled={busy || !activeDataset?.eligible_count || !raterId.trim()}>
            批量创建盲测
          </button>
        {/if}
        {#if error}<span class="error">{error}</span>{/if}
      </div>

      {#if batch}
        <div class="evaluation-results">
          <div class="metric-row">
            <div><strong>{batch.selected_count}</strong><span>所选用例</span></div>
            <div><strong>{batch.completed_count}</strong><span>完成</span></div>
            <div><strong>{batch.failed_count}</strong><span>失败</span></div>
          </div>
          {#each batch.results as result}
            <div class="result-row">
              <code>{result.case_id}</code>
              <strong class:passed={result.status === "completed"}>{result.status}</strong>
              <span>{result.score_record ? `${result.score_record.aggregate.geometric_mean.toFixed(2)} · ${result.score_record.aggregate.verdict}` : result.failed_gates.join(", ")}</span>
            </div>
          {/each}
        </div>
      {/if}

      {#if humanResults.length}
        <p class="success">本轮已提交 {humanResults.length} 份盲测记录。</p>
      {/if}
    {:else if !busy}
      <div class="empty-state" class:error={!!error}>{error || "评测目录不可用。"}</div>
    {/if}
  </section>
{/if}

{#if detailCase}
  <EvaluationCaseDetail item={detailCase} datasetLabel={activeDataset?.label ?? datasetId}
    onclose={closeCaseDetail} />
{/if}
