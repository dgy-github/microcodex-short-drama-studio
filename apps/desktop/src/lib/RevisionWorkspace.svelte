<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import type {
    RevisionComparison,
    RevisionSummary,
    RevisionWorkspace as Workspace,
    ReviewFinding,
  } from "./types";

  let { runId, onclose }: { runId: string; onclose: () => void } = $props();
  let workspace = $state<Workspace | null>(null);
  let active = $state<RevisionSummary | null>(null);
  let finding = $state<ReviewFinding | null>(null);
  let replacementText = $state("");
  let requestedChange = $state("");
  let actor = $state("operator");
  let approvalNote = $state("");
  let exportPath = $state("");
  let exportFormat = $state("json");
  let comparison = $state<RevisionComparison | null>(null);
  let busy = $state(false);
  let error = $state("");
  let message = $state("");

  async function load() {
    busy = true;
    error = "";
    try {
      workspace = await desktopApi.openRevisionWorkspace(runId);
      active = workspace.revisions.at(-1) ?? null;
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  async function selectFinding(value: ReviewFinding) {
    if (!active) return;
    finding = value;
    requestedChange = value.requested_change;
    error = "";
    try {
      const node = await desktopApi.readRevisionSpan(
        active.record.revision_id,
        value.span_ref,
      );
      replacementText = JSON.stringify(node, null, 2);
    } catch (reason) {
      error = errorMessage(reason);
    }
  }

  async function createRevision() {
    if (!active || !finding) return;
    busy = true;
    error = "";
    message = "";
    try {
      const replacement = JSON.parse(replacementText);
      const created = await desktopApi.createRevision(
        active.record.revision_id,
        finding.span_ref,
        replacement,
        requestedChange,
      );
      workspace!.revisions = [...workspace!.revisions, created];
      active = created;
      finding = null;
      replacementText = "";
      requestedChange = "";
      message = "新修订已作为不可变版本保存。";
    } catch (value) {
      error = value instanceof SyntaxError ? "替换节点不是有效 JSON。" : errorMessage(value);
    } finally {
      busy = false;
    }
  }

  async function decide(decision: "approved" | "rejected") {
    if (!active) return;
    busy = true;
    error = "";
    try {
      const updated = await desktopApi.approveRevision(
        active.record.revision_id,
        decision,
        actor,
        approvalNote,
      );
      replaceSummary(updated);
      active = updated;
      message = decision === "approved" ? "版本已批准，可导出。" : "版本已拒绝。";
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  async function comparePrevious() {
    if (!workspace || !active) return;
    const index = workspace.revisions.findIndex(
      (item) => item.record.revision_id === active!.record.revision_id,
    );
    if (index <= 0) return;
    comparison = await desktopApi.compareRevisions(
      workspace.revisions[index - 1].record.revision_id,
      active.record.revision_id,
    );
  }

  async function rollbackOrigin() {
    if (!workspace || !active || workspace.revisions.length < 2) return;
    busy = true;
    error = "";
    try {
      const created = await desktopApi.rollbackRevision(
        active.record.revision_id,
        workspace.revisions[0].record.revision_id,
        "回滚到原始生成版本",
      );
      workspace.revisions = [...workspace.revisions, created];
      active = created;
      message = "已创建新的回滚版本，旧版本未被修改。";
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  async function exportApproved() {
    if (!active) return;
    busy = true;
    error = "";
    try {
      const receipt = await desktopApi.exportRevision(
        active.record.revision_id,
        exportPath,
      );
      message = `已导出：${receipt.target_path}`;
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  function replaceSummary(updated: RevisionSummary) {
    if (!workspace) return;
    workspace.revisions = workspace.revisions.map((item) =>
      item.record.revision_id === updated.record.revision_id ? updated : item,
    );
  }

  onMount(load);
</script>

<section class="revision-workspace">
  <div class="revision-toolbar">
    <div>
      <span class="eyebrow">DIRECTED REVISION</span>
      <h3>定向修订与审批</h3>
    </div>
    <button class="ghost" onclick={onclose}>返回作品</button>
  </div>

  {#if busy && !workspace}
    <div class="empty-state">正在初始化不可变修订历史…</div>
  {:else if workspace && active}
    <div class="revision-grid">
      <aside class="revision-history">
        <strong>版本历史</strong>
        {#each workspace.revisions as revision, index}
          <button
            class:active={active.record.revision_id === revision.record.revision_id}
            onclick={() => (active = revision)}
          >
            <span>R{index} · {revision.record.kind}</span>
            <small>{revision.approval?.decision ?? "pending"}</small>
          </button>
        {/each}
        <button class="ghost" onclick={comparePrevious} disabled={workspace.revisions.length < 2}>
          对比前一版本
        </button>
        <button class="ghost danger" onclick={rollbackOrigin} disabled={workspace.revisions.length < 2}>
          回滚为新版本
        </button>
      </aside>

      <div class="revision-editor">
        <div class="revision-meta">
          <span>{active.record.revision_id}</span>
          <span>D3/D4 ROUND {active.record.round}/2</span>
          <span>{active.record.node_correspondence_count} MAPPINGS</span>
        </div>

        <h4>审查定位</h4>
        <div class="finding-list">
          {#each workspace.findings as item}
            <button
              class:active={finding?.defect_id === item.defect_id}
              onclick={() => selectFinding(item)}
            >
              <strong>{item.severity} · {item.defect_id}</strong>
              <span>{item.span_ref}</span>
              <small>{item.requested_change}</small>
            </button>
          {/each}
          {#if !workspace.findings.length}
            <p class="budget-note">当前审查没有可定位 finding。</p>
          {/if}
        </div>

        {#if finding}
          <label class="field field-wide">
            <span>修订要求</span>
            <input bind:value={requestedChange} />
          </label>
          <label class="field field-wide">
            <span>替换完整节点 JSON（必须保留 node_id）</span>
            <textarea class="json-editor" bind:value={replacementText} rows="13"></textarea>
          </label>
          <button class="primary" onclick={createRevision} disabled={busy}>
            创建不可变修订
          </button>
        {/if}

        {#if comparison}
          <div class="comparison">
            <strong>版本差异</strong>
            <span>{comparison.changed_spans.length} changed</span>
            <span>{comparison.added_spans.length} added</span>
            <span>{comparison.removed_spans.length} removed</span>
            {#each comparison.changed_spans as span}<code>{span}</code>{/each}
          </div>
        {/if}

        <div class="approval-panel">
          <label class="field"><span>审批人</span><input bind:value={actor} /></label>
          <label class="field"><span>审批备注</span><input bind:value={approvalNote} /></label>
          <button class="secondary" onclick={() => decide("approved")} disabled={busy || !!active.approval}>批准</button>
          <button class="ghost danger" onclick={() => decide("rejected")} disabled={busy || !!active.approval}>拒绝</button>
        </div>

        <div class="export-panel">
          <label class="field">
            <span>导出格式</span>
            <select bind:value={exportFormat}>
              <option value="json">JSON（原始数据）</option>
              <option value="md">Markdown（可读格式）</option>
              <option value="html">HTML（网页格式）</option>
              <option value="txt">纯文本</option>
            </select>
          </label>
          <label class="field">
            <span>导出到尚不存在的绝对路径</span>
            <input
              bind:value={exportPath}
              placeholder={`D:\\Stories\\approved-story.${exportFormat}`}
            />
          </label>
          <button class="primary" onclick={exportApproved} disabled={busy || active.approval?.decision !== "approved"}>
            导出已批准版本
          </button>
        </div>

        {#if message}<p class="success">{message}</p>{/if}
        {#if error}<p class="error">{error}</p>{/if}
      </div>
    </div>
  {:else if error}
    <div class="empty-state error">{error}</div>
  {/if}
</section>
