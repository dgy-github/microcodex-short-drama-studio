<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import type { MediaProjectRecord } from "./types";

  let projectId = $state("media_project_1");
  let sourceSpan = $state("story-package/scene-1");
  let prompt = $state("");
  let endpoint = $state("");
  let secret = $state("");
  let imageRef = $state("");
  let history = $state<MediaProjectRecord[]>([]);
  let busy = $state(false);
  let activeRun = $state("");
  let message = $state("");

  const revisions = $derived(
    history.filter((record) => record.record_type === "image_prompt_revision"),
  );
  const latestRevision = $derived(revisions.at(-1));
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
      await desktopApi.saveMediaGatewaySettings(endpoint);
      if (secret) await desktopApi.storeMediaGatewayCredential(secret);
      secret = "";
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
    await run({
      schema: "video-generation-request/v1",
      request_id: newId("vid"),
      project_id: projectId,
      image_artifact_ref: imageRef,
      story_spans: [sourceSpan.trim()],
      prompt: prompt.trim(),
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
    endpoint = saved?.endpoint ?? "";
  });
</script>

<section class="studio">
  <article>
    <h2>媒体网关</h2>
    <label>Endpoint<input bind:value={endpoint} placeholder="https://…/v1/media/generate" /></label>
    <label>Bearer secret<input bind:value={secret} type="password" autocomplete="off" /></label>
    <button onclick={saveGateway} disabled={busy || !endpoint}>保存可信配置</button>
  </article>

  <article>
    <h2>故事后续生图</h2>
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

  <article>
    <h2>图片和故事生成视频</h2>
    <label>图片 artifact reference<input bind:value={imageRef} placeholder="artifact://sha256/…" /></label>
    <button onclick={generateVideo} disabled={busy || !imageRef}>生成视频</button>
    {#if activeRun}<button class="danger" onclick={cancel}>取消当前任务</button>{/if}
    {#if message}<p role="status">{message}</p>{/if}
  </article>
</section>

<style>
  .studio{display:grid;grid-template-columns:repeat(auto-fit,minmax(280px,1fr));gap:18px}article{padding:22px;border:1px solid var(--border);border-radius:18px;background:var(--surface)}h2{margin-top:0}label{display:grid;gap:7px;margin:12px 0}input,textarea{box-sizing:border-box;width:100%;padding:11px;border:1px solid var(--border);border-radius:10px;background:var(--surface-2);color:var(--text)}button{padding:10px 14px;border:0;border-radius:10px;background:var(--accent);color:#fff}button:disabled{opacity:.45}.actions{display:flex;flex-wrap:wrap;gap:8px}.ghost{background:transparent;color:var(--text);border:1px solid var(--border)}.danger{margin-left:8px;background:#a33}p{color:var(--muted)}.history{padding-left:20px;font-size:.78rem;color:var(--muted);overflow-wrap:anywhere}.history li{margin:5px 0}
</style>
