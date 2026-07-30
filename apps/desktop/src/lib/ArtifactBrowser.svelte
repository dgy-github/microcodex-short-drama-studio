<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import RevisionWorkspace from "./RevisionWorkspace.svelte";
  import type { RunSummary, WorkflowResult } from "./types";

  let { initialRunId = null }: { initialRunId?: string | null } = $props();
  let runs = $state<RunSummary[]>([]);
  let selected = $state<RunSummary | null>(null);
  let detail = $state<WorkflowResult | null>(null);
  let busy = $state(true);
  let error = $state("");
  let revising = $state(false);
  let reading = $state(false);

  async function loadRuns() {
    busy = true;
    error = "";
    try {
      runs = await desktopApi.listRuns();
      const initial =
        runs.find((run) => run.run_id === initialRunId) ?? runs[0];
      if (initial) await selectRun(initial);
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

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
    <button class="ghost" onclick={loadRuns} disabled={busy}>刷新</button>
  </div>

  {#if busy && !runs.length}
    <div class="empty-state">正在读取本地作品库…</div>
  {:else if error && !runs.length}
    <div class="empty-state error">{error}</div>
  {:else if !runs.length}
    <div class="empty-state">还没有完成的 advisory 故事包。</div>
  {:else}
    <div class="artifact-layout">
      <div class="run-list">
        {#each runs as run, index}
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
    role="presentation"
    onclick={(event) => {
      if (event.target === event.currentTarget) closeReader();
    }}
  >
    <div
      class="story-reader"
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
        <button class="ghost" type="button" onclick={closeReader} aria-label="关闭完整故事">
          关闭
        </button>
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
        <div class="story-episode-list">
          {#each detail.package.episodes ?? [] as episode, index}
            {@const episodeScenes = scenesForEpisode(episode.node_id)}
            <article class="story-episode">
              <header>
                <span>EPISODE {episode.index ?? index + 1}</span>
                <h4>第 {episode.index ?? index + 1} 集</h4>
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
