<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
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
  <section class="panel evaluation-panel">
    <div class="panel-heading">
      <div>
        <span class="eyebrow">BLIND HUMAN REVIEW · {assignmentIndex + 1}/{assignments.length}</span>
        <h2>{activeAssignment.alias}</h2>
      </div>
      <button class="ghost" onclick={leaveBlind} disabled={busy}>退出盲测</button>
    </div>

    <div class="blind-context">
      <div><small>故事前提</small><p>{activeAssignment.prompt}</p></div>
      <details>
        <summary>查看盲化故事包</summary>
        <pre>{JSON.stringify(activeAssignment.artifact, null, 2)}</pre>
      </details>
    </div>

    <div class="score-grid">
      {#each activeAssignment.dimensions as dimension, index}
        <article class="score-card">
          <div class="score-title">
            <div><strong>{dimension.name}</strong><small>{dimension.dimension_id}</small></div>
            <select
              aria-label={`${dimension.name}分数`}
              value={scores[index]?.score ?? 3}
              onchange={(event) => updateScore(index, "score", event.currentTarget.value)}
            >
              {#each [1, 2, 3, 4, 5] as value}<option {value}>{value} 分</option>{/each}
            </select>
          </div>
          <p>{dimension.ask}</p>
          <div class="score-anchors">
            <span><b>1</b>{dimension.anchors["1"]}</span>
            <span><b>3</b>{dimension.anchors["3"]}</span>
            <span><b>5</b>{dimension.anchors["5"]}</span>
          </div>
          <textarea
            rows="2"
            placeholder="评分理由（必填）"
            value={scores[index]?.reason ?? ""}
            oninput={(event) => updateScore(index, "reason", event.currentTarget.value)}
          ></textarea>
          <select
            aria-label={`${dimension.name}证据位置`}
            value={scores[index]?.span_refs[0] ?? ""}
            onchange={(event) => updateScore(index, "span", event.currentTarget.value)}
          >
            {#each activeAssignment.allowed_spans as span}<option value={span}>{span}</option>{/each}
          </select>
        </article>
      {/each}
    </div>
    <div class="action-row">
      <button
        class="primary"
        onclick={submitBlind}
        disabled={busy || scores.some((item) => !item.reason.trim() || !item.span_refs[0])}
      >提交本份盲测</button>
      <span class="shield">不展示 split、生成模型、历史分数和缺陷键</span>
      {#if error}<span class="error">{error}</span>{/if}
    </div>
  </section>
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
      <div class="empty-state">评测目录不可用。</div>
    {/if}
  </section>
{/if}

{#if detailCase}
  <div
    class="case-detail-backdrop"
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) closeCaseDetail();
    }}
  >
    <div
      class="case-detail-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="case-detail-title"
    >
      <div class="case-detail-heading">
        <div>
          <span class="eyebrow">EVALUATION CASE DETAIL</span>
          <h3 id="case-detail-title">{detailCase.case_id}</h3>
        </div>
        <button class="ghost" type="button" onclick={closeCaseDetail} aria-label="关闭用例详情">
          关闭
        </button>
      </div>
      <p class="case-detail-copy">{detailCase.label}</p>
      <dl class="case-detail-grid">
        <div><dt>数据集</dt><dd>{activeDataset?.label ?? datasetId}</dd></div>
        <div><dt>题材</dt><dd>{detailCase.genre}</dd></div>
        <div><dt>难度</dt><dd>{detailCase.difficulty ?? "真实运行"}</dd></div>
        <div><dt>数据分区</dt><dd>{detailCase.split ?? "online"}</dd></div>
        <div>
          <dt>运行状态</dt>
          <dd class:ready={detailCase.eligible}>
            {detailCase.eligible ? "READY · 可运行" : "NO ARTIFACT · 暂不可运行"}
          </dd>
        </div>
      </dl>
      <p class="case-detail-footnote">按 Esc 或点击遮罩区域关闭。</p>
    </div>
  </div>
{/if}
