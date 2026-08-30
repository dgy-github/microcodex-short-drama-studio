<script lang="ts">
  import type { BlindAssignment, HumanDimensionInput } from "./types";

  let {
    assignment,
    assignmentIndex,
    assignmentCount,
    scores,
    busy,
    error,
    onleave,
    onupdate,
    onsubmit,
  }: {
    assignment: BlindAssignment;
    assignmentIndex: number;
    assignmentCount: number;
    scores: HumanDimensionInput[];
    busy: boolean;
    error: string;
    onleave: () => void;
    onupdate: (index: number, key: "score" | "reason" | "span", value: string) => void;
    onsubmit: () => void;
  } = $props();
</script>

<section class="panel evaluation-panel">
  <div class="panel-heading">
    <div>
      <span class="eyebrow">BLIND HUMAN REVIEW · {assignmentIndex + 1}/{assignmentCount}</span>
      <h2>{assignment.alias}</h2>
    </div>
    <button class="ghost" onclick={onleave} disabled={busy}>退出盲测</button>
  </div>

  <div class="blind-context">
    <div><small>故事前提</small><p>{assignment.prompt}</p></div>
    <details><summary>查看盲化故事包</summary><pre>{JSON.stringify(assignment.artifact, null, 2)}</pre></details>
  </div>

  <div class="score-grid">
    {#each assignment.dimensions as dimension, index}
      <article class="score-card">
        <div class="score-title">
          <div><strong>{dimension.name}</strong><small>{dimension.dimension_id}</small></div>
          <select aria-label={`${dimension.name}分数`} value={scores[index]?.score ?? 3}
            onchange={(event) => onupdate(index, "score", event.currentTarget.value)}>
            {#each [1, 2, 3, 4, 5] as value}<option {value}>{value} 分</option>{/each}
          </select>
        </div>
        <p>{dimension.ask}</p>
        <div class="score-anchors">
          <span><b>1</b>{dimension.anchors["1"]}</span>
          <span><b>3</b>{dimension.anchors["3"]}</span>
          <span><b>5</b>{dimension.anchors["5"]}</span>
        </div>
        <textarea rows="2" placeholder="评分理由（必填）" value={scores[index]?.reason ?? ""}
          oninput={(event) => onupdate(index, "reason", event.currentTarget.value)}></textarea>
        <select aria-label={`${dimension.name}证据位置`} value={scores[index]?.span_refs[0] ?? ""}
          onchange={(event) => onupdate(index, "span", event.currentTarget.value)}>
          {#each assignment.allowed_spans as span}<option value={span}>{span}</option>{/each}
        </select>
      </article>
    {/each}
  </div>
  <div class="action-row">
    <button class="primary" onclick={onsubmit}
      disabled={busy || scores.some((item) => !item.reason.trim() || !item.span_refs[0])}>
      提交本份盲测
    </button>
    <span class="shield">不展示 split、生成模型、历史分数和缺陷键</span>
    {#if error}<span class="error">{error}</span>{/if}
  </div>
</section>
