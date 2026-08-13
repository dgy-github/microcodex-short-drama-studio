<script lang="ts">
  import { onMount } from "svelte";
  import { desktopApi, errorMessage } from "./api";
  import type {
    CredentialAuditEvent,
    CredentialStatus,
    ProviderRouteSettings,
    ProviderSoakResult,
  } from "./types";

  type Provider = CredentialStatus["provider"];
  const labels: Record<Provider, { name: string; role: string }> = {
    deepseek: { name: "DeepSeek", role: "故事生成" },
    aliyun_bailian: { name: "阿里云百炼", role: "独立审查" },
  };
  let statuses = $state<Record<Provider, boolean>>({
    deepseek: false,
    aliyun_bailian: false,
  });
  let secrets = $state<Record<Provider, string>>({
    deepseek: "",
    aliyun_bailian: "",
  });
  let routes = $state<Record<Provider, ProviderRouteSettings>>({
    deepseek: {
      schema: "desktop-provider-route/v1",
      provider: "deepseek",
      profile: "default",
      endpoint: "",
      model: "",
      thinking_disabled: false,
      source: "default",
      record_id: null,
      updated_at_unix_ms: null,
    },
    aliyun_bailian: {
      schema: "desktop-provider-route/v1",
      provider: "aliyun_bailian",
      profile: "default",
      endpoint: "",
      model: "",
      thinking_disabled: true,
      source: "default",
      record_id: null,
      updated_at_unix_ms: null,
    },
  });
  let busy = $state<Provider | "soak" | null>(null);
  let message = $state("");
  let lastAudit = $state<CredentialAuditEvent | null>(null);
  let soakIterations = $state(5);
  let soakResult = $state<ProviderSoakResult | null>(null);

  async function refresh() {
    const values = await Promise.all(
      (Object.keys(labels) as Provider[]).map(async (provider) => ({
        status: await desktopApi.credentialStatus(provider),
        route: await desktopApi.providerRoute(provider),
      })),
    );
    for (const value of values) {
      statuses[value.status.provider] = value.status.configured;
      routes[value.route.provider] = value.route;
    }
    const audit = await desktopApi.credentialAudit();
    lastAudit = audit.at(-1) ?? null;
  }

  function updateRoute(provider: Provider, field: "endpoint" | "model", value: string) {
    routes[provider] = { ...routes[provider], [field]: value };
  }

  async function saveRoute(provider: Provider) {
    busy = provider;
    message = "";
    try {
      const route = routes[provider];
      routes[provider] = await desktopApi.saveProviderRoute(
        provider,
        route.endpoint,
        route.model,
      );
      message = `${labels[provider].name} 地址与模型已保存`;
    } catch (value) {
      message = errorMessage(value);
    } finally {
      busy = null;
    }
  }

  async function save(provider: Provider) {
    busy = provider;
    message = "";
    try {
      const value = await desktopApi.storeCredential(provider, secrets[provider]);
      statuses[provider] = value.configured;
      secrets[provider] = "";
      message = `${labels[provider].name} 凭据已安全保存`;
      const audit = await desktopApi.credentialAudit();
      lastAudit = audit.at(-1) ?? null;
    } catch (value) {
      secrets[provider] = "";
      message = errorMessage(value);
    } finally {
      busy = null;
    }
  }

  async function remove(provider: Provider) {
    busy = provider;
    message = "";
    try {
      const value = await desktopApi.deleteCredential(provider);
      statuses[provider] = value.configured;
      message = `${labels[provider].name} 凭据已删除`;
      const audit = await desktopApi.credentialAudit();
      lastAudit = audit.at(-1) ?? null;
    } catch (value) {
      message = errorMessage(value);
    } finally {
      busy = null;
    }
  }

  async function checkHealth(provider: Provider) {
    busy = provider;
    message = "";
    try {
      const health = await desktopApi.checkProviderHealth(provider);
      message = `${labels[provider].name} · ${health.model} 连接正常`;
    } catch (value) {
      message = errorMessage(value);
    } finally {
      busy = null;
    }
  }

  async function runSoak() {
    busy = "soak";
    message = "";
    soakResult = null;
    try {
      soakResult = await desktopApi.runProviderSoak(soakIterations);
      message =
        soakResult.status === "ready"
          ? "双供应商稳定性检查全部通过"
          : "稳定性检查已完成，存在失败请求";
    } catch (value) {
      message = errorMessage(value);
    } finally {
      busy = null;
    }
  }

  onMount(() => refresh().catch((value) => (message = errorMessage(value))));
</script>

<section class="panel credential-panel">
  <div class="panel-heading">
    <div>
      <span class="eyebrow">模型连接</span>
      <h2>本机凭据保险箱</h2>
    </div>
    <span class="shield">Windows Credential Manager</span>
  </div>

  <div class="provider-list">
    {#each Object.entries(labels) as [provider, meta]}
      <article class="provider-card">
        <div class="provider-card-heading">
          <div class="provider-mark">{meta.name.slice(0, 1)}</div>
          <div class="provider-copy">
            <strong>{meta.name}</strong>
            <span>{meta.role}</span>
          </div>
          <span class:configured={statuses[provider as Provider]} class="connection">
            {statuses[provider as Provider] ? "凭据已配置" : "凭据未配置"}
          </span>
          <span class="route-source">
            {routes[provider as Provider].source === "user" ? "自定义路由" : "默认路由"}
          </span>
        </div>
        <div class="provider-route-grid">
          <label class="field">
            <span>Endpoint</span>
            <input
              type="url"
              placeholder="https://…/chat/completions"
              value={routes[provider as Provider].endpoint}
              oninput={(event) => updateRoute(provider as Provider, "endpoint", event.currentTarget.value)}
            />
          </label>
          <label class="field">
            <span>Model ID</span>
            <input
              placeholder="模型 ID"
              value={routes[provider as Provider].model}
              oninput={(event) => updateRoute(provider as Provider, "model", event.currentTarget.value)}
            />
          </label>
          <button
            class="secondary"
            onclick={() => saveRoute(provider as Provider)}
            disabled={busy !== null || !routes[provider as Provider].endpoint || !routes[provider as Provider].model}
          >保存地址</button>
        </div>
        <div class="provider-secret-row">
          <input
            class="secret-input"
            type="password"
            autocomplete="new-password"
            placeholder="粘贴新的 API Key"
            bind:value={secrets[provider as Provider]}
          />
          <button
            class="secondary"
            onclick={() => save(provider as Provider)}
            disabled={busy !== null || !secrets[provider as Provider]}
          >保存凭据</button>
          <button
            class="ghost danger"
            onclick={() => remove(provider as Provider)}
            disabled={busy !== null || !statuses[provider as Provider]}
          >删除凭据</button>
          <button
            class="ghost"
            onclick={() => checkHealth(provider as Provider)}
            disabled={busy !== null || !statuses[provider as Provider]}
          >健康检查</button>
        </div>
      </article>
    {/each}
  </div>
  <div class="provider-soak">
    <div>
      <span class="eyebrow">LIVE PROVIDER SOAK</span>
      <strong>双供应商稳定性检查</strong>
      <small>这是付费操作：总请求数为轮数 × 2，结果只保留计数与延迟。</small>
    </div>
    <label class="compact-field">
      每供应商轮数
      <input type="number" min="3" max="20" bind:value={soakIterations} />
    </label>
    <button
      class="primary"
      onclick={runSoak}
      disabled={busy !== null || !statuses.deepseek || !statuses.aliyun_bailian || soakIterations < 3 || soakIterations > 20}
    >运行稳定性检查</button>
  </div>
  {#if soakResult}
    <div class="soak-result">
      {#each soakResult.providers as provider}
        <article class:degraded={provider.status === "degraded"}>
          <strong>{labels[provider.provider].name} · {provider.model}</strong>
          <span>{provider.successful_requests}/{soakResult.iterations_per_provider} 成功</span>
          <small>
            延迟 {provider.min_latency_ms}/{provider.average_latency_ms}/{provider.max_latency_ms} ms
          </small>
        </article>
      {/each}
    </div>
  {/if}
  {#if message}<p class="inline-message">{message}</p>{/if}
  {#if lastAudit}
    <p class="budget-note">
      最近审计 #{lastAudit.sequence} · {lastAudit.provider} · {lastAudit.action}
    </p>
  {/if}
</section>
