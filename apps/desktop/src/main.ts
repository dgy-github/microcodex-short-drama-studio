import { mount } from "svelte";
import App from "./App.svelte";
import "./app.css";

async function bootstrap(): Promise<void> {
  if (import.meta.env.VITE_WDIO === "true") {
    await import("@wdio/tauri-plugin");
  }
  mount(App, {
    target: document.getElementById("app")!,
  });
}

void bootstrap();
