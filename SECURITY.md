# Security policy

## Reporting

不要在公开 issue 中披露凭据、未授权内容或可利用漏洞。请通过仓库维护者
提供的私密安全渠道报告；若尚未配置，请先联系仓库所有者。

## Boundaries

- Rust 拥有可信存储、供应商访问、权限、预算和进程执行。
- Python sidecar 只接收类型化能力，不获得任意 shell。
- Svelte 不直连 sidecar 或模型供应商。
- API key 只放本地 `.env` 或部署密钥存储。
- 不摄取未授权的受保护故事。
