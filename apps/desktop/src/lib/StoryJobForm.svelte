<script lang="ts">
  import { desktopApi, errorMessage } from "./api";
  import { onDestroy, onMount } from "svelte";
  import RunConsole from "./RunConsole.svelte";
  import type {
    GenrePackOption,
    RunSnapshot,
    StoryJob,
    StoryJobPreview,
  } from "./types";

  let {
    oncompleted = () => {},
  }: {
    oncompleted?: (runId: string) => void;
  } = $props();
  let premise = $state(
    "停电后的老旧商场里，一名维修工发现故障电梯中被困的是二十年前离开的父亲。",
  );
  let genre = $state("family, suspense");
  let audience = $state("25-45");
  let genrePack = $state("");
  let genrePacks = $state<GenrePackOption[]>([]);
  let constraintProfile = $state("short-vertical-v1");
  let episodes = $state(6);
  let minutes = $state(2);
  let limits = $state("不美化遗弃行为\n不使用超自然解释");
  let maxTokens = $state(180000);
  let maxCost = $state(1200);
  let deadline = $state(900);
  let busy = $state(false);
  let preview = $state<StoryJobPreview | null>(null);
  let error = $state("");
  let snapshot = $state<RunSnapshot | null>(null);
  let cancelling = $state(false);
  let syncBusy = false;
  let notifiedRunId: string | null = null;
  let timer: ReturnType<typeof setInterval> | null = null;

  function recommendedTokensFor(episodeCount: number) {
    return Math.max(180000, 90000 + episodeCount * 15000);
  }

  let recommendedTokenBudget = $derived(recommendedTokensFor(episodes));
  let runActive = $derived(
    snapshot?.status === "accepted" || snapshot?.status === "running",
  );

  function buildJob(): StoryJob {
    return {
      schema: "story-job/v1",
      job_id: `job_desktop_${Date.now()}`,
      content_form: "scripted_short_drama",
      input: premise,
      genre_mode: "fixed",
      allowed_genres: genre.split(",").map((item) => item.trim()).filter(Boolean),
      genre_pack_id: genrePack,
      constraint_profile_id: constraintProfile,
      audience,
      format: { episodes, minutes_per_episode: minutes },
      content_limits: limits.split("\n").map((item) => item.trim()).filter(Boolean),
      budget: {
        max_tokens: maxTokens,
        max_cny_fen: maxCost,
        deadline_seconds: deadline,
      },
    };
  }

  function applyGenrePack() {
    const selected = genrePacks.find((option) => option.pack_id === genrePack);
    if (!selected) return;
    genre = selected.genre;
    audience = selected.default_audience;
  }

  async function loadGenrePacks() {
    try {
      genrePacks = await desktopApi.listGenrePacks();
      const preferred =
        genrePacks.find((option) => option.pack_id === "family-grounded-v1") ??
        genrePacks[0];
      if (!preferred) throw new Error("类型包注册表为空。");
      genrePack = preferred.pack_id;
      applyGenrePack();
    } catch (value) {
      error = errorMessage(value);
    }
  }

  function applyConstraintProfile() {
    episodes = constraintProfile === "long-serial-v1" ? 40 : 6;
    maxTokens = recommendedTokensFor(episodes);
  }

  async function validate() {
    busy = true;
    error = "";
    preview = null;
    try {
      preview = await desktopApi.validateStoryJob(buildJob());
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  async function start() {
    if (busy || runActive) {
      error = "当前故事任务仍在运行，不会重复创建。";
      return;
    }
    busy = true;
    error = "";
    preview = null;
    try {
      const job = buildJob();
      preview = await desktopApi.validateStoryJob(job);
      snapshot = await desktopApi.startRun(job);
      beginSync();
    } catch (value) {
      error = errorMessage(value);
    } finally {
      busy = false;
    }
  }

  function beginSync() {
    if (timer !== null) clearInterval(timer);
    timer = setInterval(sync, 800);
  }

  async function sync() {
    if (syncBusy || !snapshot) return;
    if (["completed", "failed", "cancelled"].includes(snapshot.status)) {
      if (timer !== null) clearInterval(timer);
      timer = null;
      return;
    }
    syncBusy = true;
    try {
      snapshot = await desktopApi.syncRun();
      if (
        snapshot.status === "completed" &&
        notifiedRunId !== snapshot.run_id
      ) {
        notifiedRunId = snapshot.run_id;
        oncompleted(snapshot.run_id);
      }
    } catch (value) {
      error = errorMessage(value);
    } finally {
      syncBusy = false;
    }
  }

  async function cancel() {
    cancelling = true;
    error = "";
    try {
      snapshot = await desktopApi.cancelRun();
    } catch (value) {
      error = errorMessage(value);
    } finally {
      cancelling = false;
    }
  }

  onDestroy(() => {
    if (timer !== null) clearInterval(timer);
  });
  onMount(loadGenrePacks);
</script>

<section class="panel form-panel">
  <div class="panel-heading">
    <div>
      <span class="eyebrow">新建项目</span>
      <h2>把一句想法变成制作任务</h2>
    </div>
    <span class="status-chip neutral">剧本短剧</span>
  </div>

  <label class="field field-wide">
    <span>故事前提</span>
    <textarea bind:value={premise} rows="5" maxlength="1200"></textarea>
  </label>

  <div class="field-grid">
    <label class="field">
      <span>类型包</span>
      <select bind:value={genrePack} onchange={applyGenrePack} disabled={!genrePacks.length}>
        {#if !genrePacks.length}
          <option value="">正在加载类型包…</option>
        {/if}
        {#each genrePacks as option}
          <option value={option.pack_id}>{option.display_name}</option>
        {/each}
      </select>
    </label>
    <label class="field">
      <span>集数约束</span>
      <select bind:value={constraintProfile} onchange={applyConstraintProfile}>
        <option value="short-vertical-v1">短篇 6–12 集</option>
        <option value="long-serial-v1">长篇 40–80 集</option>
      </select>
    </label>
    <label class="field">
      <span>题材标签</span>
      <input bind:value={genre} />
      <small>英文逗号分隔</small>
    </label>
    <label class="field">
      <span>核心受众</span>
      <input bind:value={audience} />
    </label>
    <label class="field">
      <span>集数</span>
      <input type="number" min="1" max="100" bind:value={episodes} />
    </label>
    <label class="field">
      <span>单集分钟</span>
      <input type="number" min="1" max="30" bind:value={minutes} />
    </label>
  </div>

  <label class="field field-wide">
    <span>内容边界</span>
    <textarea bind:value={limits} rows="3"></textarea>
  </label>

  <div class="field-grid three">
    <label class="field">
      <span>Token 上限</span>
      <input type="number" min="1" bind:value={maxTokens} />
      <small>
        当前 {episodes} 集完整流程建议不少于
        {recommendedTokenBudget.toLocaleString()} Token
      </small>
    </label>
    <label class="field">
      <span>预算（分）</span>
      <input type="number" min="0" bind:value={maxCost} />
    </label>
    <label class="field">
      <span>时限（秒）</span>
      <input type="number" min="1" bind:value={deadline} />
    </label>
  </div>

  <div class="action-row">
    <button class="primary" onclick={validate} disabled={busy || !genrePack}>
      {busy ? "正在校验…" : "校验故事任务"}
    </button>
    <button class="secondary" onclick={start} disabled={busy || !genrePack || runActive}>
      {busy
        ? "正在准备…"
        : runActive
          ? "任务运行中 · 已防重复"
          : "启动 17-task 流程"}
    </button>
    {#if preview}
      <p class="success">
        已通过 Rust 校验 · {preview.episodes} 集 × {preview.minutes_per_episode} 分钟
      </p>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
  </div>

  {#if snapshot}
    <RunConsole {snapshot} {cancelling} oncancel={cancel} />
  {/if}
</section>
