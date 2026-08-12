<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import RevisionWorkspace from "./RevisionWorkspace.svelte";
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

  // Reading mode enhancements
  let fullscreenMode = $state(false);
  let fontSize = $state(16); // Base font size in px
  let currentEpisodeIndex = $state(0);
  let bookmarks = $state<Set<string>>(new Set()); // Store episode node_ids

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
    let result = [...runs];

    // Apply search filter
    if (searchQuery.trim()) {
      const query = searchQuery.toLowerCase();
      result = result.filter((run) => {
        const logline = (run.logline || "").toLowerCase();
        const runId = run.run_id.toLowerCase();
        const models = `${run.generation_model} ${run.review_model}`.toLowerCase();
        return logline.includes(query) || runId.includes(query) || models.includes(query);
      });
    }

    // Apply status filter
    if (filterStatus !== "all") {
      result = result.filter((run) => {
        // Assuming runs with task_count < 17 are failed
        const isCompleted = run.task_count >= 17;
        return filterStatus === "completed" ? isCompleted : !isCompleted;
      });
    }

    // Apply sorting
    if (sortBy === "date") {
      result.sort((a, b) => b.completed_at_unix_ms - a.completed_at_unix_ms);
    } else if (sortBy === "name") {
      result.sort((a, b) => (a.logline || "").localeCompare(b.logline || ""));
    }

    filteredRuns = result;
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
    fullscreenMode = false;
    currentEpisodeIndex = 0;
  }

  function toggleFullscreen() {
    fullscreenMode = !fullscreenMode;
  }

  function increaseFontSize() {
    if (fontSize < 24) fontSize += 2;
  }

  function decreaseFontSize() {
    if (fontSize > 12) fontSize -= 2;
  }

  function resetFontSize() {
    fontSize = 16;
  }

  function jumpToEpisode(index: number) {
    currentEpisodeIndex = index;
    const episodeElement = document.getElementById(`episode-${index}`);
    if (episodeElement) {
      episodeElement.scrollIntoView({ behavior: "smooth", block: "start" });
    }
  }

  function toggleBookmark(nodeId: string | undefined) {
    if (!nodeId) return;
    if (bookmarks.has(nodeId)) {
      bookmarks.delete(nodeId);
    } else {
      bookmarks.add(nodeId);
    }
    bookmarks = new Set(bookmarks);
  }

  function isBookmarked(nodeId?: string) {
    return nodeId ? bookmarks.has(nodeId) : false;
  }

  function scenesForEpisode(nodeId?: string) {
    if (!nodeId) return [];
    return (detail?.package.scenes ?? []).filter(
      (scene) => scene.episode_ref === `story-package/${nodeId}`,
    );
  }

  function speakerName(reference?: string) {
    const nodeId = reference?.split("/").at(-1);
    return (
      detail?.package.characters?.find((character) => character.node_id === nodeId)
        ?.name ?? "角色"
    );
  }

  function speakerIndex(reference?: string) {
    const nodeId = reference?.split("/").at(-1);
    const index =
      detail?.package.characters?.findIndex(
        (character) => character.node_id === nodeId,
      ) ?? -1;
    return Math.max(index, 0);
  }

  function avatarInitial(name?: string) {
    return name?.trim().slice(0, 1) || "角";
  }

  function avatarTone(index: number) {
    return `avatar-tone-${index % 6}`;
  }

  function completedAt(timestamp: number) {
    return new Date(timestamp).toLocaleString("zh-CN", { hour12: false });
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

  async function batchDelete() {
    if (selectedRunIds.size === 0) return;

    // TODO: 实现批量删除功能
    // 需要后端 API 支持删除运行记录
    batchMessage = "批量删除功能待实现（需要后端支持）";
  }

  onMount(loadRuns);
</script>

<svelte:window
  onkeydown={(event) => {
    if (event.key === "Escape" && reading) closeReader();
  }}
/>

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
        <button
          class="ghost danger"
          onclick={batchDelete}
          disabled={batchBusy || selectedRunIds.size === 0}
        >
          批量删除
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
  <div
    class="story-reader-backdrop"
    class:fullscreen={fullscreenMode}
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) closeReader();
    }}
  >
    <div
      class="story-reader"
      class:fullscreen={fullscreenMode}
      style="font-size: {fontSize}px;"
      role="dialog"
      aria-modal="true"
      aria-labelledby="story-reader-title"
    >
      <header class="story-reader-heading">
        <div>
          <span class="eyebrow">COMPLETE STORY PACKAGE</span>
          <h2 id="story-reader-title">完整故事</h2>
          <small>{detail.package.package_id} · {selected.episode_count} 集</small>
        </div>
        <div class="reader-controls">
          <button
            class="ghost control-btn"
            type="button"
            onclick={decreaseFontSize}
            aria-label="减小字体"
            title="减小字体"
          >
            A-
          </button>
          <button
            class="ghost control-btn"
            type="button"
            onclick={resetFontSize}
            aria-label="重置字体"
            title="重置字体大小"
          >
            A
          </button>
          <button
            class="ghost control-btn"
            type="button"
            onclick={increaseFontSize}
            aria-label="增大字体"
            title="增大字体"
          >
            A+
          </button>
          <button
            class="ghost control-btn"
            type="button"
            onclick={toggleFullscreen}
            aria-label={fullscreenMode ? "退出全屏" : "全屏模式"}
            title={fullscreenMode ? "退出全屏" : "全屏模式"}
          >
            {fullscreenMode ? "⊟" : "⊡"}
          </button>
          <button class="ghost" type="button" onclick={closeReader} aria-label="关闭完整故事">
            关闭
          </button>
        </div>
      </header>

      <section class="story-reader-summary">
        <span>故事梗概</span>
        <p>{detail.package.logline?.text ?? "未提供故事梗概"}</p>
        {#if detail.package.promise}
          <small>
            {detail.package.promise.genre ?? "未分类"} ·
            {detail.package.promise.audience ?? "未指定受众"} ·
            {detail.package.promise.tone ?? "未指定基调"}
          </small>
        {/if}
      </section>

      <section class="story-reader-section">
        <h3>人物</h3>
        <div class="story-character-grid">
          {#each detail.package.characters ?? [] as character, characterIndex}
            <article class="character-card">
              <div class={`cartoon-avatar ${avatarTone(characterIndex)}`}>
                <span>{avatarInitial(character.name)}</span>
              </div>
              <div class="character-card-copy">
                <h4>{character.name ?? "未命名角色"}</h4>
                <dl>
                  <div><dt>欲望</dt><dd>{character.desire ?? "—"}</dd></div>
                  <div><dt>恐惧</dt><dd>{character.fear ?? "—"}</dd></div>
                  <div><dt>矛盾</dt><dd>{character.contradiction ?? "—"}</dd></div>
                  <div><dt>秘密</dt><dd>{character.secret ?? "—"}</dd></div>
                  <div><dt>变化</dt><dd>{character.change ?? "—"}</dd></div>
                </dl>
              </div>
            </article>
          {/each}
        </div>
      </section>

      <section class="story-reader-section">
        <h3>分集正文</h3>

        <!-- Quick Jump Navigation -->
        <div class="episode-navigator">
          <span class="nav-label">快速跳转：</span>
          <div class="episode-jump-buttons">
            {#each detail.package.episodes ?? [] as episode, index}
              <button
                class="episode-jump-btn"
                class:active={currentEpisodeIndex === index}
                class:bookmarked={isBookmarked(episode.node_id)}
                onclick={() => jumpToEpisode(index)}
                title={isBookmarked(episode.node_id) ? "已加书签" : ""}
              >
                {index + 1}
                {#if isBookmarked(episode.node_id)}<span class="bookmark-indicator">★</span>{/if}
              </button>
            {/each}
          </div>
        </div>

        <div class="story-episode-list">
          {#each detail.package.episodes ?? [] as episode, index}
            {@const episodeScenes = scenesForEpisode(episode.node_id)}
            <article class="story-episode" id="episode-{index}">
              <header>
                <div class="episode-header-left">
                  <span>EPISODE {episode.index ?? index + 1}</span>
                  <h4>第 {episode.index ?? index + 1} 集</h4>
                </div>
                <button
                  class="ghost bookmark-btn"
                  onclick={() => toggleBookmark(episode.node_id)}
                  aria-label={isBookmarked(episode.node_id) ? "移除书签" : "添加书签"}
                  title={isBookmarked(episode.node_id) ? "移除书签" : "添加书签"}
                >
                  {isBookmarked(episode.node_id) ? "★" : "☆"}
                </button>
              </header>
              <div class="episode-outline">
                <p><b>开场</b>{episode.opening_state ?? "—"}</p>
                <p><b>冲突</b>{episode.conflict ?? "—"}</p>
                <p><b>转折</b>{episode.turn ?? "—"}</p>
                <p><b>钩子</b>{episode.end_hook?.text ?? "—"}</p>
              </div>
              {#if episodeScenes.length}
                <div class="episode-scenes">
                  {#each episodeScenes as scene, sceneIndex}
                    <section>
                      <h5>场景 {sceneIndex + 1} · {scene.location ?? "未指定地点"}</h5>
                      <div class="script-lines">
                        {#each scene.lines ?? [] as line}
                          {#if line.kind === "dialogue"}
                            {@const characterIndex = speakerIndex(line.speaker)}
                            {@const characterName = speakerName(line.speaker)}
                            <div class:reverse={characterIndex % 2 === 1} class="comic-dialogue">
                              <div class={`cartoon-avatar compact ${avatarTone(characterIndex)}`}>
                                <span>{avatarInitial(characterName)}</span>
                              </div>
                              <div class="speech-bubble">
                                <strong>{characterName}</strong>
                                <p>{line.text ?? ""}</p>
                                {#if line.subtext}<small>心声 · {line.subtext}</small>{/if}
                              </div>
                            </div>
                          {:else}
                            <div class="storyboard-caption">
                              <span>镜头</span>
                              <p>{line.text ?? ""}</p>
                            </div>
                          {/if}
                        {/each}
                      </div>
                    </section>
                  {/each}
                </div>
              {:else}
                <p class="outline-only">
                  该集只有分集梗概；这是旧版故事包，未生成本集完整剧本场景。
                </p>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    </div>
  </div>
{/if}
