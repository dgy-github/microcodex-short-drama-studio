import { resolve } from "node:path";
import { mkdirSync } from "node:fs";

const e2eProfile = resolve(
  process.env.RUNNER_TEMP ?? "target-e2e",
  `wdio-webview2-${process.pid}`,
);
mkdirSync(e2eProfile, { recursive: true });

const appBinaryPath = resolve(
  "src-tauri",
  "target-e2e",
  "debug",
  process.platform === "win32" ? "story-desktop.exe" : "story-desktop",
);

export const config = {
  runner: "local",
  specs: ["./e2e-tauri/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [{
    browserName: "tauri",
    // WebView2 uses the Edge driver on Windows. A private profile prevents
    // the runner's default profile from racing with another browser process
    // and avoids the DevToolsActivePort startup failure in CI.
    "ms:edgeOptions": {
      args: [
        `--user-data-dir=${e2eProfile}`,
        "--no-first-run",
        "--disable-gpu",
      ],
    },
    "tauri:options": {
      application: appBinaryPath,
    },
  }],
  services: [["tauri", {
    appBinaryPath,
    driverProvider: "external",
    autoInstallTauriDriver: true,
  }]],
  logLevel: "info",
  bail: 0,
  waitforTimeout: 15_000,
  connectionRetryTimeout: 120_000,
  connectionRetryCount: 1,
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: {
    ui: "bdd",
    timeout: 60_000,
  },
};
