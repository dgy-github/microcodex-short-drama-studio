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
      mediaGatewaySettings: vi.fn(),
      saveMediaGatewaySettings: vi.fn(),
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
});
