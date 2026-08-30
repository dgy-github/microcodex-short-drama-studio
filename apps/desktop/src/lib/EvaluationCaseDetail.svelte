<script lang="ts">
  import type { EvaluationCase } from "./types";

  let {
    item,
    datasetLabel,
    onclose,
  }: {
    item: EvaluationCase;
    datasetLabel: string;
    onclose: () => void;
  } = $props();
</script>

<div class="case-detail-backdrop" role="presentation"
  onclick={(event) => { if (event.target === event.currentTarget) onclose(); }}>
  <div class="case-detail-dialog" role="dialog" aria-modal="true" aria-labelledby="case-detail-title">
    <div class="case-detail-heading">
      <div>
        <span class="eyebrow">EVALUATION CASE DETAIL</span>
        <h3 id="case-detail-title">{item.case_id}</h3>
      </div>
      <button class="ghost" type="button" onclick={onclose} aria-label="关闭用例详情">关闭</button>
    </div>
    <p class="case-detail-copy">{item.label}</p>
    <dl class="case-detail-grid">
      <div><dt>数据集</dt><dd>{datasetLabel}</dd></div>
      <div><dt>题材</dt><dd>{item.genre}</dd></div>
      <div><dt>难度</dt><dd>{item.difficulty ?? "真实运行"}</dd></div>
      <div><dt>数据分区</dt><dd>{item.split ?? "online"}</dd></div>
      <div>
        <dt>运行状态</dt>
        <dd class:ready={item.eligible}>
          {item.eligible ? "READY · 可运行" : "NO ARTIFACT · 暂不可运行"}
        </dd>
      </div>
    </dl>
    <p class="case-detail-footnote">按 Esc 或点击遮罩区域关闭。</p>
  </div>
</div>
