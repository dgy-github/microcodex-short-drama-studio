<script lang="ts">
  import type { RunSummary, WorkflowResult } from "./types";

  let { result, run, onclose }: {
    result: WorkflowResult;
    run: RunSummary;
    onclose: () => void;
  } = $props();
  let fullscreen = $state(false);
  let fontSize = $state(16);
  let currentEpisode = $state(0);
  let bookmarks = $state<Set<string>>(new Set());

  function jumpToEpisode(index: number) {
    currentEpisode = index;
    document.getElementById(`episode-${index}`)?.scrollIntoView({ behavior: "smooth", block: "start" });
  }

  function toggleBookmark(nodeId?: string) {
    if (!nodeId) return;
    if (bookmarks.has(nodeId)) bookmarks.delete(nodeId);
    else bookmarks.add(nodeId);
    bookmarks = new Set(bookmarks);
  }

  function isBookmarked(nodeId?: string) {
    return nodeId ? bookmarks.has(nodeId) : false;
  }

  function scenesForEpisode(nodeId?: string) {
    return nodeId
      ? (result.package.scenes ?? []).filter((scene) => scene.episode_ref === `story-package/${nodeId}`)
      : [];
  }

  function speaker(reference?: string) {
    const nodeId = reference?.split("/").at(-1);
    const index = result.package.characters?.findIndex((item) => item.node_id === nodeId) ?? -1;
    return {
      name: result.package.characters?.find((item) => item.node_id === nodeId)?.name ?? "角色",
      index: Math.max(index, 0),
    };
  }

  const avatarTone = (index: number) => `avatar-tone-${index % 6}`;
  const avatarInitial = (name?: string) => name?.trim().slice(0, 1) || "角";
</script>

<svelte:window onkeydown={(event) => { if (event.key === "Escape") onclose(); }} />

<div class="story-reader-backdrop" class:fullscreen role="presentation"
  onclick={(event) => { if (event.target === event.currentTarget) onclose(); }}>
  <div class="story-reader" class:fullscreen style="font-size: {fontSize}px;"
    role="dialog" aria-modal="true" aria-labelledby="story-reader-title">
    <header class="story-reader-heading">
      <div>
        <span class="eyebrow">COMPLETE STORY PACKAGE</span>
        <h2 id="story-reader-title">完整故事</h2>
        <small>{result.package.package_id} · {run.episode_count} 集</small>
      </div>
      <div class="reader-controls">
        <button class="ghost control-btn" onclick={() => (fontSize = Math.max(12, fontSize - 2))}
          aria-label="减小字体" title="减小字体">A-</button>
        <button class="ghost control-btn" onclick={() => (fontSize = 16)}
          aria-label="重置字体" title="重置字体大小">A</button>
        <button class="ghost control-btn" onclick={() => (fontSize = Math.min(24, fontSize + 2))}
          aria-label="增大字体" title="增大字体">A+</button>
        <button class="ghost control-btn" onclick={() => (fullscreen = !fullscreen)}
          aria-label={fullscreen ? "退出全屏" : "全屏模式"} title={fullscreen ? "退出全屏" : "全屏模式"}>
          {fullscreen ? "⊟" : "⊡"}
        </button>
        <button class="ghost" onclick={onclose} aria-label="关闭完整故事">关闭</button>
      </div>
    </header>

    <section class="story-reader-summary">
      <span>故事梗概</span>
      <p>{result.package.logline?.text ?? "未提供故事梗概"}</p>
      {#if result.package.promise}
        <small>{result.package.promise.genre ?? "未分类"} · {result.package.promise.audience ?? "未指定受众"} · {result.package.promise.tone ?? "未指定基调"}</small>
      {/if}
    </section>

    <section class="story-reader-section">
      <h3>人物</h3>
      <div class="story-character-grid">
        {#each result.package.characters ?? [] as character, index}
          <article class="character-card">
            <div class={`cartoon-avatar ${avatarTone(index)}`}><span>{avatarInitial(character.name)}</span></div>
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
      <div class="episode-navigator">
        <span class="nav-label">快速跳转：</span>
        <div class="episode-jump-buttons">
          {#each result.package.episodes ?? [] as episode, index}
            <button class="episode-jump-btn" class:active={currentEpisode === index}
              class:bookmarked={isBookmarked(episode.node_id)} onclick={() => jumpToEpisode(index)}>
              {index + 1}{#if isBookmarked(episode.node_id)}<span class="bookmark-indicator">★</span>{/if}
            </button>
          {/each}
        </div>
      </div>

      <div class="story-episode-list">
        {#each result.package.episodes ?? [] as episode, index}
          {@const scenes = scenesForEpisode(episode.node_id)}
          <article class="story-episode" id="episode-{index}">
            <header>
              <div class="episode-header-left"><span>EPISODE {episode.index ?? index + 1}</span><h4>第 {episode.index ?? index + 1} 集</h4></div>
              <button class="ghost bookmark-btn" onclick={() => toggleBookmark(episode.node_id)}
                aria-label={isBookmarked(episode.node_id) ? "移除书签" : "添加书签"}>
                {isBookmarked(episode.node_id) ? "★" : "☆"}
              </button>
            </header>
            <div class="episode-outline">
              <p><b>开场</b>{episode.opening_state ?? "—"}</p><p><b>冲突</b>{episode.conflict ?? "—"}</p>
              <p><b>转折</b>{episode.turn ?? "—"}</p><p><b>钩子</b>{episode.end_hook?.text ?? "—"}</p>
            </div>
            {#if scenes.length}
              <div class="episode-scenes">
                {#each scenes as scene, sceneIndex}
                  <section>
                    <h5>场景 {sceneIndex + 1} · {scene.location ?? "未指定地点"}</h5>
                    <div class="script-lines">
                      {#each scene.lines ?? [] as line}
                        {#if line.kind === "dialogue"}
                          {@const character = speaker(line.speaker)}
                          <div class:reverse={character.index % 2 === 1} class="comic-dialogue">
                            <div class={`cartoon-avatar compact ${avatarTone(character.index)}`}><span>{avatarInitial(character.name)}</span></div>
                            <div class="speech-bubble"><strong>{character.name}</strong><p>{line.text ?? ""}</p>{#if line.subtext}<small>心声 · {line.subtext}</small>{/if}</div>
                          </div>
                        {:else}
                          <div class="storyboard-caption"><span>镜头</span><p>{line.text ?? ""}</p></div>
                        {/if}
                      {/each}
                    </div>
                  </section>
                {/each}
              </div>
            {:else}
              <p class="outline-only">该集只有分集梗概；这是旧版故事包，未生成本集完整剧本场景。</p>
            {/if}
          </article>
        {/each}
      </div>
    </section>
  </div>
</div>
