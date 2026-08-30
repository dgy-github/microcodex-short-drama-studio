<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import type { MediaProjectRecord } from "./types";

  let projectId = $state("media_project_1");
  let sourceSpan = $state("story-package/scene-1");
  let prompt = $state("");
  let endpoint = $state("");
  let coarseEndpoint = $state("");
  let fineEndpoint = $state("");
  let secret = $state("");
  let coarseSecret = $state("");
  let fineSecret = $state("");
  let videoTier = $state<"coarse" | "fine">("coarse");
  let storyAlignment = $state(0);
  let identityConsistency = $state(0);
  let motionQuality = $state(0);
  let continuity = $state(0);
  let artifactFree = $state(0);
  let imageRef = $state("");
  let history = $state<MediaProjectRecord[]>([]);
  let busy = $state(false);
  let activeRun = $state("");
  let message = $state("");

  const revisions = $derived(
    history.filter((record) => record.record_type === "image_prompt_revision"),
  );
  const latestRevision = $derived(revisions.at(-1));
  const videoQualityPassed = $derived(
    storyAlignment >= 0.8 && identityConsistency >= 0.85 && motionQuality >= 0.8
      && continuity >= 0.8 && artifactFree >= 0.85,
  );
  const newId = (prefix: string) =>
    `${prefix}_${crypto.randomUUID().replaceAll("-", "")}`;

  async function refresh() {
    try {
      history = await desktopApi.readMediaProjectHistory(projectId);
      message = `已读取 ${history.length} 条不可改写记录`;
    } catch (error) {
      message = errorMessage(error);
    }
  }

  async function saveGateway() {
    busy = true;
    try {
      const coarse = coarseEndpoint || endpoint;
      const fine = fineEndpoint || coarse;
      await desktopApi.saveMediaGenerationRoutes(coarse, fine);
      if (secret) await desktopApi.storeMediaGatewayCredential(secret);
      if (coarseSecret) await desktopApi.storeMediaGatewayCredential(coarseSecret, "coarse");
      if (fineSecret) await desktopApi.storeMediaGatewayCredential(fineSecret, "fine");
      secret = "";
      coarseSecret = "";
      fineSecret = "";
      message = "网关已保存；密钥仅存于 Windows Credential Manager";
    } catch (error) {
      message = errorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function savePromptRevision() {
    if (!prompt.trim() || !sourceSpan.trim()) return;
    busy = true;
    try {
      await desktopApi.appendMediaPromptRevision({
        schema: "image-prompt-revision/v1",
        project_id: projectId,
        revision_id: newId("prompt"),
        parent_revision_id: latestRevision?.record_id ?? null,
        prompt: prompt.trim(),
        source_spans: [sourceSpan.trim()],
      });
      await refresh();
    } catch (error) {
      message = errorMessage(error);
    } finally {
      busy = false;
    }
  }

  async function generateImage() {
    if (!latestRevision) {
      message = "请先保存提示词版本";
      return;
    }
    await run({
      schema: "image-generation-request/v1",
      request_id: newId("img"),
      project_id: projectId,
      prompt_revision_id: latestRevision.record_id,
      prompt: latestRevision.data.prompt,
      source_spans: latestRevision.data.source_spans,
    });
  }

  async function generateVideo() {
    if (!imageRef || !prompt.trim() || !sourceSpan.trim()) return;
    if (videoTier === "fine" && !videoQualityPassed) {
      message = "质量门禁未通过；请先补段或重新粗生成";
      return;
    }
    await run({
      schema: "video-generation-request/v1",
      request_id: newId("vid"),
      project_id: projectId,
      image_artifact_ref: imageRef,
      story_spans: [sourceSpan.trim()],
      prompt: prompt.trim(),
      generation_tier: videoTier,
    });
  }

  async function run(request: Record<string, unknown>) {
    busy = true;
    activeRun = newId("media_run");
    try {
      await desktopApi.appendMediaGenerationRequest(request);
      const outcome = await desktopApi.startMediaRun(activeRun, request);
      if (outcome.result) {
        if (outcome.result.kind === "Image") imageRef = outcome.result.content_ref;
        message = `${outcome.result.mime_type} 已保留 · ${outcome.result.cost_cny_fen} 分`;
      }
      await refresh();
    } catch (error) {
      message = errorMessage(error);
    } finally {
      busy = false;
      activeRun = "";
    }
  }

  async function cancel() {
    if (activeRun) await desktopApi.cancelMediaRun(activeRun);
  }

  onMount(async () => {
    const saved = await desktopApi.mediaGatewaySettings().catch(() => null);
    if (!endpoint) endpoint = saved?.endpoint ?? "";
    if (!coarseEndpoint) coarseEndpoint = saved?.coarse_endpoint ?? saved?.endpoint ?? "";
    if (!fineEndpoint) fineEndpoint = saved?.fine_endpoint ?? saved?.endpoint ?? "";
  });
</script>

<section class="studio">
  <header class="hero">
    <div><span class="eyebrow">MEDIA AGENT WORKSPACE</span><h2>故事媒体工坊</h2><p>从故事证据到候选、评估与精生成，全程保留不可改写记录。</p></div>
    <span class="status-dot">● {busy ? "运行中" : "就绪"}</span>
  </header>

  <article class="gateway">
    <h3>可信媒体网关</h3>
    <label>兼容默认 Endpoint<input bind:value={endpoint} placeholder="https://…/v1/media/generate" /></label>
    <label>Wan 粗生成 Endpoint<input bind:value={coarseEndpoint} placeholder="https://…/wan/generate" /></label>
    <label>Kling 精生成 Endpoint<input bind:value={fineEndpoint} placeholder="https://…/kling/generate" /></label>
    <label>Bearer secret<input bind:value={secret} type="password" autocomplete="off" /></label>
    <label>Wan secret<input bind:value={coarseSecret} type="password" autocomplete="off" /></label>
    <label>Kling secret<input bind:value={fineSecret} type="password" autocomplete="off" /></label>
    <button onclick={saveGateway} disabled={busy || !(coarseEndpoint || endpoint) || !fineEndpoint}>保存可信配置</button>
  </article>

  <article class="image-flow">
    <div class="section-title"><div><span class="step-number">01</span><h3>故事生图 Agent</h3></div><span class="provider">候选 → 评估 → 定稿</span></div>
    <div class="pipeline" aria-label="生图流程"><span>镜头拆解</span><i>→</i><span>候选生成</span><i>→</i><span>一致性评估</span><i>→</i><span>局部修订</span><i>→</i><span class="final">定稿</span></div>
    <label>项目 ID<input bind:value={projectId} /></label>
    <label>故事位置<input bind:value={sourceSpan} /></label>
    <label>图片提示词<textarea bind:value={prompt} rows="5"></textarea></label>
    <div class="actions">
      <button onclick={savePromptRevision} disabled={busy}>保存新版本</button>
      <button onclick={generateImage} disabled={busy}>按当前版本生成图片</button>
      <button class="ghost" onclick={refresh}>读取历史</button>
    </div>
    <p>提示词版本 {revisions.length} · 总记录 {history.length}</p>
    <ol class="history" aria-label="媒体项目历史">
      {#each history.slice().reverse().slice(0, 8) as record}
        <li><strong>#{record.seq}</strong> {record.record_type} · {record.record_id}</li>
      {/each}
    </ol>
  </article>

  <article class="video-flow">
    <div class="section-title"><div><span class="step-number">02</span><h3>故事生视频 Agent</h3></div><span class="provider">Wan 粗生成 · Kling 精生成</span></div>
    <div class="pipeline video" aria-label="生视频流程"><span>Wan 粗生成</span><i>→</i><span>裁剪</span><i>→</i><span>补段</span><i>→</i><span>质量门禁</span><i>→</i><span class="final">Kling 精生成</span></div>
    <label>图片 artifact reference<input bind:value={imageRef} placeholder="artifact://sha256/…" /></label>
    <label>生成阶段<select bind:value={videoTier}><option value="coarse">Wan 粗生成</option><option value="fine">Kling 精生成</option></select></label>
    {#if videoTier === "fine"}
      <fieldset class="quality-gate">
        <legend>精生成质量门禁</legend>
        <label>故事符合度<input type="number" min="0" max="1" step="0.01" bind:value={storyAlignment} /></label>
        <label>人物一致性<input type="number" min="0" max="1" step="0.01" bind:value={identityConsistency} /></label>
        <label>动作质量<input type="number" min="0" max="1" step="0.01" bind:value={motionQuality} /></label>
        <label>镜头连续性<input type="number" min="0" max="1" step="0.01" bind:value={continuity} /></label>
        <label>画面无伪影<input type="number" min="0" max="1" step="0.01" bind:value={artifactFree} /></label>
        <p class:passed={videoQualityPassed}>{videoQualityPassed ? "✓ 门禁通过" : "未达标，禁止精生成"}</p>
      </fieldset>
    {/if}
    <button onclick={generateVideo} disabled={busy || !imageRef || (videoTier === "fine" && !videoQualityPassed)}>生成视频</button>
    {#if activeRun}<button class="danger" onclick={cancel}>取消当前任务</button>{/if}
    {#if message}<p role="status">{message}</p>{/if}
  </article>
</section>

<style>
  .studio{display:grid;grid-template-columns:1fr 1fr;gap:16px}.hero{grid-column:1/-1;display:flex;justify-content:space-between;align-items:center;padding:22px 24px;border:1px solid var(--border);border-radius:18px;background:radial-gradient(700px 180px at 10% 0,rgba(88,166,255,.18),transparent),var(--surface)}.hero h2{margin:4px 0;font-size:1.55rem}.hero p{margin:0}.eyebrow{font:600 .68rem ui-monospace;color:#58a6ff;letter-spacing:.14em}.status-dot{padding:7px 11px;border:1px solid rgba(126,231,135,.3);border-radius:999px;color:#7ee787;background:rgba(126,231,135,.07);font-size:.78rem}article{padding:20px;border:1px solid var(--border);border-radius:16px;background:linear-gradient(145deg,rgba(88,166,255,.035),rgba(126,231,135,.02)),var(--surface)}.gateway{grid-column:1/-1}.section-title,.section-title>div{display:flex;align-items:center;justify-content:space-between;gap:9px}.section-title h3{margin:0}.step-number{display:grid;place-items:center;width:29px;height:29px;border-radius:9px;background:#58a6ff;color:#07111d;font-weight:800}.provider{font-size:.72rem;color:#7ee787}.pipeline{display:flex;align-items:center;gap:6px;margin:16px 0;padding:10px;border:1px solid var(--border);border-radius:12px;background:var(--surface-2);overflow:auto}.pipeline span{white-space:nowrap;padding:6px 8px;border-radius:7px;border:1px solid var(--border);font-size:.7rem}.pipeline i{color:var(--muted);font-style:normal}.pipeline .final{border-color:rgba(126,231,135,.45);color:#7ee787}h3{margin-top:0}label{display:grid;gap:7px;margin:12px 0;font-size:.8rem;color:var(--muted)}input,textarea{box-sizing:border-box;width:100%;padding:11px;border:1px solid var(--border);border-radius:9px;background:var(--surface-2);color:var(--text)}button{padding:10px 14px;border:0;border-radius:9px;background:#58a6ff;color:#07111d;font-weight:650}button:disabled{opacity:.45}.actions{display:flex;flex-wrap:wrap;gap:8px}.ghost{background:transparent;color:var(--text);border:1px solid var(--border)}.danger{margin-left:8px;background:#f0883e;color:#1b0d02}p{color:var(--muted)}.quality-gate{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:0 10px;margin:14px 0;padding:12px;border:1px solid rgba(240,136,62,.35);border-radius:12px;background:rgba(240,136,62,.06)}.quality-gate legend{padding:0 6px;color:#f0883e;font-size:.78rem}.quality-gate label{margin:5px 0}.quality-gate p{grid-column:1/-1;margin:8px 0 0;color:#f0883e}.quality-gate p.passed{color:#7ee787}.history{padding-left:20px;font-size:.72rem;color:var(--muted);overflow-wrap:anywhere}.history li{margin:5px 0}@media(max-width:850px){.studio{grid-template-columns:1fr}.hero,.gateway{grid-column:auto}.hero{align-items:flex-start;gap:16px}.pipeline{padding-bottom:13px}}
</style>
