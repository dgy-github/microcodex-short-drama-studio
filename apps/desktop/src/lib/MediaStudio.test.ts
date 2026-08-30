import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import { beforeEach, describe, expect, it, vi } from "vitest";
import MediaStudio from "./MediaStudio.svelte";
import { desktopApi } from "./api";

vi.mock("./api", async () => {
  const actual = await vi.importActual<typeof import("./api")>("./api");
  return {
    ...actual,
    desktopApi: {
      readMediaProjectHistory: vi.fn(),
      appendMediaPromptRevision: vi.fn(),
      appendMediaGenerationRequest: vi.fn(),
      startMediaRun: vi.fn(),
      cancelMediaRun: vi.fn(),
      validateMediaTimelineRequest: vi.fn(),
      executeMediaTimeline: vi.fn(),
      mediaGatewaySettings: vi.fn(),
      saveMediaGatewaySettings: vi.fn(),
      saveMediaGenerationRoutes: vi.fn(),
      storeMediaGatewayCredential: vi.fn(),
    },
  };
});

describe("MediaStudio", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.stubGlobal("crypto", { randomUUID: () => "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa" });
    vi.mocked(desktopApi.readMediaProjectHistory).mockResolvedValue([]);
    vi.mocked(desktopApi.mediaGatewaySettings).mockResolvedValue(null);
  });

  it("saves a story-grounded prompt revision", async () => {
    render(MediaStudio);
    await fireEvent.input(screen.getByLabelText("图片提示词"), {
      target: { value: "雨夜站台，人物回头" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "保存新版本" }));
    await waitFor(() => expect(desktopApi.appendMediaPromptRevision).toHaveBeenCalled());
    expect(desktopApi.appendMediaPromptRevision).toHaveBeenCalledWith(
      expect.objectContaining({
        schema: "image-prompt-revision/v1",
        source_spans: ["story-package/scene-1"],
      }),
    );
  });

  it("persists an image request before starting its run", async () => {
    vi.mocked(desktopApi.readMediaProjectHistory).mockResolvedValue([
      {
        schema: "media-project-record/v1", seq: 1, project_id: "media_project_1",
        record_id: "prompt_1", record_type: "image_prompt_revision",
        data: { prompt: "雨夜站台", source_spans: ["story-package/scene-1"] },
      },
    ]);
    vi.mocked(desktopApi.startMediaRun).mockResolvedValue({
      schema: "desktop-media-run-result/v1", run_id: "media_run", status: "cancelled", result: null,
    });
    render(MediaStudio);
    await fireEvent.click(screen.getByRole("button", { name: "读取历史" }));
    await screen.findByText(/提示词版本 1/);
    await fireEvent.click(screen.getByRole("button", { name: "按当前版本生成图片" }));
    await waitFor(() => expect(desktopApi.startMediaRun).toHaveBeenCalled());
    expect(vi.mocked(desktopApi.appendMediaGenerationRequest).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(desktopApi.startMediaRun).mock.invocationCallOrder[0]);
  });

  it("saves separate Wan and Kling routes", async () => {
    render(MediaStudio);
    await waitFor(() => expect(desktopApi.mediaGatewaySettings).toHaveBeenCalled());
    await fireEvent.input(screen.getByLabelText("Wan 粗生成 Endpoint"), {
      target: { value: "https://media.example/wan/generate" },
    });
    await fireEvent.input(screen.getByLabelText("Kling 精生成 Endpoint"), {
      target: { value: "https://media.example/kling/generate" },
    });
    await fireEvent.click(screen.getByRole("button", { name: "保存可信配置" }));
    await waitFor(() => expect(desktopApi.saveMediaGenerationRoutes).toHaveBeenCalledWith(
      "https://media.example/wan/generate", "https://media.example/kling/generate",
    ));
  });

  it("sends fine generation tier for Kling video", async () => {
    vi.mocked(desktopApi.startMediaRun).mockResolvedValue({
      schema: "desktop-media-run-result/v1", run_id: "media_run", status: "cancelled", result: null,
    });
    render(MediaStudio);
    await fireEvent.input(screen.getByLabelText("图片提示词"), { target: { value: "镜头缓慢推近" } });
    await fireEvent.input(screen.getByLabelText("图片 artifact reference"), {
      target: { value: `artifact://sha256/${"a".repeat(64)}` },
    });
    await fireEvent.change(screen.getByLabelText("生成阶段"), { target: { value: "fine" } });
    for (const label of ["故事符合度", "人物一致性", "动作质量", "镜头连续性", "画面无伪影"]) {
      await fireEvent.input(screen.getByLabelText(label), { target: { value: "0.9" } });
    }
    await fireEvent.click(screen.getByRole("button", { name: "生成视频" }));
    await waitFor(() => expect(desktopApi.appendMediaGenerationRequest).toHaveBeenCalledWith(
      expect.objectContaining({ schema: "video-generation-request/v1", generation_tier: "fine" }),
    ));
  });

  it("validates an artifact-only editing timeline through Rust", async () => {
    render(MediaStudio);
    await fireEvent.input(screen.getByLabelText("视频 artifact"), {
      target: { value: `artifact://sha256/${"b".repeat(64)}` },
    });
    await fireEvent.input(screen.getByLabelText("开始秒数"), { target: { value: "1" } });
    await fireEvent.input(screen.getByLabelText("结束秒数"), { target: { value: "4" } });
    await fireEvent.click(screen.getByRole("button", { name: "校验剪辑时间线" }));
    await waitFor(() => expect(desktopApi.validateMediaTimelineRequest).toHaveBeenCalledWith(
      expect.objectContaining({
        schema: "desktop-media-timeline-request/v1",
        clips: [{ content_ref: `artifact://sha256/${"b".repeat(64)}`, start_seconds: 1, end_seconds: 4 }],
      }),
    ));
  });

  it("executes the timeline through Rust and reports the retained artifact", async () => {
    vi.mocked(desktopApi.executeMediaTimeline).mockResolvedValue({
      schema: "media-artifact-ref/v1", project_id: "media_project_1", request_id: "edit_1",
      kind: "video", mime_type: "video/mp4", content_ref: `artifact://sha256/${"c".repeat(64)}`,
      content_sha256: "c".repeat(64), byte_len: 12,
    });
    render(MediaStudio);
    await fireEvent.input(screen.getByLabelText("视频 artifact"), {
      target: { value: `artifact://sha256/${"b".repeat(64)}` },
    });
    await fireEvent.click(screen.getByRole("button", { name: "执行裁剪与拼接" }));
    await waitFor(() => expect(desktopApi.executeMediaTimeline).toHaveBeenCalled());
    expect(screen.getByRole("status")).toHaveTextContent(/剪辑完成并已保留/);
  });
});
