import { resolve } from "node:path";
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
