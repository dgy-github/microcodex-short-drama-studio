<script lang="ts">
  import { onMount } from "svelte";
  import ArtifactBrowser from "./lib/ArtifactBrowser.svelte";
  import { desktopApi } from "./lib/api";
  import CredentialPanel from "./lib/CredentialPanel.svelte";
  import EvaluationCenter from "./lib/EvaluationCenter.svelte";
  import StoryJobForm from "./lib/StoryJobForm.svelte";

  let active = $state<"create" | "library" | "evaluation" | "settings">("create");
  let runCount = $state<number | null>(null);
  let latestCompletedRunId = $state<string | null>(null);

  function openCompletedRun(runId: string) {
    latestCompletedRunId = runId;
    active = "library";
    desktopApi.listRuns().then(
      (runs) => (runCount = runs.length),
      () => (runCount = null),
    );
  }

  onMount(() => {
    Promise.all([
      desktopApi.credentialStatus("deepseek"),
      desktopApi.credentialStatus("aliyun_bailian"),
    ]).then((statuses) => {
      if (statuses.some((status) => !status.configured)) active = "settings";
    });
    desktopApi.listRuns().then(
      (runs) => (runCount = runs.length),
      () => (runCount = null),
    );
  });
</script>

<svelte:head><title>MicrocodeX 短剧工作室</title></svelte:head>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark">M</div>
      <div><strong>MicrocodeX</strong><span>短剧工作室</span></div>
    </div>
    <nav aria-label="主导航">
      <button class:active={active === "create"} onclick={() => (active = "create")}>
        <span>✦</span>创作台
      </button>
      <button class:active={active === "library"} onclick={() => (active = "library")}>
        <span>▤</span>作品库
      </button>
      <button class:active={active === "evaluation"} onclick={() => (active = "evaluation")}>
        <span>◎</span>评测中心
      </button>
      <button class:active={active === "settings"} onclick={() => (active = "settings")}>
        <span>◉</span>模型配置
      </button>
    </nav>
    <div class="sidebar-note">
      <span class="pulse"></span>
      <div><strong>P5 Desktop</strong><small>本地可信边界</small></div>
    </div>
  </aside>

  <main>
    <header class="topbar">
      <div>
        <span class="eyebrow">STORY OPERATING SYSTEM</span>
        <h1>
          {active === "create" ? "创作台" : active === "library" ? "作品库" : active === "evaluation" ? "评测中心" : "模型配置"}
        </h1>
      </div>
      <div class="topbar-state">
        <span></span>LOCAL · {runCount === null ? "CONNECTING" : `${runCount} RUNS`}
      </div>
    </header>

    <div class="content">
      {#if active === "create"}
        <div class="hero">
          <div>
            <p>从故事前提开始</p>
            <h2>让每一个选择<br />都能被审查与回看。</h2>
          </div>
          <div class="hero-index"><span>01</span><small>任务定义</small></div>
        </div>
        <StoryJobForm oncompleted={openCompletedRun} />
      {:else if active === "library"}
        <ArtifactBrowser initialRunId={latestCompletedRunId} />
      {:else if active === "evaluation"}
        <EvaluationCenter />
      {:else}
        <CredentialPanel />
      {/if}
    </div>
  </main>
</div>
