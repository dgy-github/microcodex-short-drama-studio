import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import StoryJobForm from "./StoryJobForm.svelte";
import * as api from "./api";

// Mock the API module
vi.mock("./api", () => ({
  desktopApi: {
    listGenrePacks: vi.fn(),
    validateStoryJob: vi.fn(),
    startRun: vi.fn(),
    syncRun: vi.fn(),
    cancelRun: vi.fn(),
  },
  errorMessage: vi.fn((error) => String(error)),
}));

describe("StoryJobForm", () => {
  const mockGenrePacks = [
    {
      pack_id: "family-grounded-v1",
      display_name: "家庭现实",
      genre: "family, drama",
      default_audience: "25-45",
    },
    {
      pack_id: "suspense-thriller-v1",
      display_name: "悬疑惊悚",
      genre: "suspense, thriller",
      default_audience: "18-35",
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.desktopApi.listGenrePacks).mockResolvedValue(mockGenrePacks);
  });

  afterEach(() => {
    vi.clearAllTimers();
  });

  describe("初始化", () => {
    it("应该加载类型包列表", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        expect(api.desktopApi.listGenrePacks).toHaveBeenCalled();
      });
    });

    it("应该默认选中家庭现实类型包", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        const select = screen.getByLabelText("类型包") as HTMLSelectElement;
        expect(select.value).toBe("family-grounded-v1");
      });
    });

    it("应该显示默认故事前提", () => {
      render(StoryJobForm);

      const textarea = screen.getByLabelText("故事前提") as HTMLTextAreaElement;
      expect(textarea.value).toContain("停电后的老旧商场");
    });
  });

  describe("类型包切换", () => {
    it("应该在切换类型包时更新题材和受众", async () => {
      render(StoryJobForm);

      // 等待类型包加载完成并且表单字段渲染
      await waitFor(() => {
        expect(api.desktopApi.listGenrePacks).toHaveBeenCalled();
      });

      // 等待表单字段出现
      await waitFor(() => {
        expect(screen.getByLabelText("类型包")).toBeInTheDocument();
        expect(screen.getByLabelText("题材标签")).toBeInTheDocument();
        expect(screen.getByLabelText("核心受众")).toBeInTheDocument();
      }, { timeout: 5000 });

      const genrePackSelect = screen.getByLabelText("类型包") as HTMLSelectElement;
      const genreInput = screen.getByLabelText("题材标签") as HTMLInputElement;
      const audienceInput = screen.getByLabelText("核心受众") as HTMLInputElement;

      // 切换到悬疑惊悚
      await fireEvent.change(genrePackSelect, { target: { value: "suspense-thriller-v1" } });

      await waitFor(() => {
        expect(genreInput.value).toBe("suspense, thriller");
        expect(audienceInput.value).toBe("18-35");
      });
    });
  });

  describe("约束配置切换", () => {
    it("应该在切换到长篇时调整集数", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByLabelText("集数约束")).toBeInTheDocument();
      });

      const constraintSelect = screen.getByLabelText("集数约束") as HTMLSelectElement;
      const episodesInput = screen.getByLabelText("集数") as HTMLInputElement;

      // 默认是短篇，6集
      expect(episodesInput.value).toBe("6");

      // 切换到长篇
      await fireEvent.change(constraintSelect, { target: { value: "long-serial-v1" } });

      expect(episodesInput.value).toBe("40");
    });

    it("应该根据集数自动调整 Token 建议", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByLabelText("集数")).toBeInTheDocument();
      });

      const episodesInput = screen.getByLabelText("集数") as HTMLInputElement;

      // 修改集数
      await fireEvent.input(episodesInput, { target: { value: "10" } });

      // 检查建议文本是否更新（10集建议 240000 tokens）
      expect(screen.getByText(/当前 10 集完整流程建议不少于/)).toBeInTheDocument();
    });
  });

  describe("任务校验", () => {
    it("应该成功校验有效的故事任务", async () => {
      const mockPreview = {
        episodes: 6,
        minutes_per_episode: 2,
      };

      vi.mocked(api.desktopApi.validateStoryJob).mockResolvedValue(mockPreview);

      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByText("校验故事任务")).toBeInTheDocument();
      });

      const validateButton = screen.getByText("校验故事任务");
      await fireEvent.click(validateButton);

      await waitFor(() => {
        expect(api.desktopApi.validateStoryJob).toHaveBeenCalled();
        expect(screen.getByText(/已通过 Rust 校验/)).toBeInTheDocument();
      }, { timeout: 3000 });
    });

    it("应该显示校验失败错误", async () => {
      vi.mocked(api.desktopApi.validateStoryJob).mockRejectedValue(
        new Error("集数不能为零")
      );
      vi.mocked(api.errorMessage).mockReturnValue("集数不能为零");

      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByText("校验故事任务")).toBeInTheDocument();
      });

      const validateButton = screen.getByText("校验故事任务");
      await fireEvent.click(validateButton);

      await waitFor(() => {
        expect(screen.getByText("集数不能为零")).toBeInTheDocument();
      }, { timeout: 3000 });
    });
  });

  describe("启动任务", () => {
    it("应该成功启动故事生成任务", async () => {
      const mockPreview = { episodes: 6, minutes_per_episode: 2 };
      const mockSnapshot = {
        run_id: "run_test123",
        status: "running",
        progress: { completed: 2, total: 17 },
      };

      vi.mocked(api.desktopApi.validateStoryJob).mockResolvedValue(mockPreview);
      vi.mocked(api.desktopApi.startRun).mockResolvedValue(mockSnapshot);

      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByText(/启动 17-task 流程/)).toBeInTheDocument();
      }, { timeout: 5000 });

      const startButton = screen.getByText(/启动 17-task 流程/);
      await fireEvent.click(startButton);

      await waitFor(() => {
        expect(api.desktopApi.validateStoryJob).toHaveBeenCalled();
        expect(api.desktopApi.startRun).toHaveBeenCalled();
      }, { timeout: 3000 });
    });

    it("应该在任务运行时禁用启动按钮", async () => {
      const mockPreview = { episodes: 6, minutes_per_episode: 2 };
      const mockSnapshot = {
        run_id: "run_test123",
        status: "running",
        progress: { completed: 2, total: 17 },
        budget: {
          max_tokens: 180000,
          max_cny_fen: 1200,
          deadline_seconds: 900,
          consumed_tokens: 5000,
          consumed_cny_fen: null,
        },
      };

      vi.mocked(api.desktopApi.validateStoryJob).mockResolvedValue(mockPreview);
      vi.mocked(api.desktopApi.startRun).mockResolvedValue(mockSnapshot);

      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByText(/启动 17-task 流程/)).toBeInTheDocument();
      }, { timeout: 5000 });

      const startButton = screen.getByText(/启动 17-task 流程/) as HTMLButtonElement;
      await fireEvent.click(startButton);

      await waitFor(() => {
        // 查找包含"任务运行中"的按钮并验证它被禁用
        const buttons = screen.getAllByRole("button");
        const runningButton = buttons.find(btn =>
          btn.textContent?.includes("任务运行中")
        ) as HTMLButtonElement;

        expect(runningButton).toBeDefined();
        expect(runningButton.disabled).toBe(true);
      }, { timeout: 3000 });
    });
  });

  describe("Token 预算计算", () => {
    it("应该为 6 集计算正确的推荐 Token", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        // 6集: max(180000, 90000 + 6 * 15000) = 180000
        expect(screen.getByText(/当前 6 集完整流程建议不少于 180,000 Token/)).toBeInTheDocument();
      });
    });

    it("应该为 40 集计算正确的推荐 Token", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByLabelText("集数约束")).toBeInTheDocument();
      });

      const constraintSelect = screen.getByLabelText("集数约束") as HTMLSelectElement;
      await fireEvent.change(constraintSelect, { target: { value: "long-serial-v1" } });

      await waitFor(() => {
        // 40集: max(180000, 90000 + 40 * 15000) = 690000
        expect(screen.getByText(/当前 40 集完整流程建议不少于 690,000 Token/)).toBeInTheDocument();
      });
    });
  });

  describe("表单输入", () => {
    it("应该允许修改故事前提", async () => {
      render(StoryJobForm);

      const textarea = screen.getByLabelText("故事前提") as HTMLTextAreaElement;
      const newPremise = "一个神秘的包裹改变了平凡职员的生活";

      await fireEvent.input(textarea, { target: { value: newPremise } });

      expect(textarea.value).toBe(newPremise);
    });

    it("应该允许修改集数和单集分钟", async () => {
      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByLabelText("集数")).toBeInTheDocument();
      });

      const episodesInput = screen.getByLabelText("集数") as HTMLInputElement;
      const minutesInput = screen.getByLabelText("单集分钟") as HTMLInputElement;

      await fireEvent.input(episodesInput, { target: { value: "12" } });
      await fireEvent.input(minutesInput, { target: { value: "3" } });

      expect(episodesInput.value).toBe("12");
      expect(minutesInput.value).toBe("3");
    });

    it("应该允许修改内容边界", async () => {
      render(StoryJobForm);

      const limitsTextarea = screen.getByLabelText("内容边界") as HTMLTextAreaElement;
      const newLimits = "不涉及暴力\n不涉及政治";

      await fireEvent.input(limitsTextarea, { target: { value: newLimits } });

      expect(limitsTextarea.value).toBe(newLimits);
    });
  });

  describe("错误处理", () => {
    it("应该处理类型包加载失败", async () => {
      vi.mocked(api.desktopApi.listGenrePacks).mockRejectedValue(
        new Error("网络错误")
      );
      vi.mocked(api.errorMessage).mockReturnValue("网络错误");

      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByText("网络错误")).toBeInTheDocument();
      }, { timeout: 3000 });
    });

    it("应该处理启动任务失败", async () => {
      vi.mocked(api.desktopApi.listGenrePacks).mockResolvedValue(mockGenrePacks);
      vi.mocked(api.desktopApi.validateStoryJob).mockResolvedValue({
        episodes: 6,
        minutes_per_episode: 2,
      });
      vi.mocked(api.desktopApi.startRun).mockRejectedValue(
        new Error("Token 余额不足")
      );
      vi.mocked(api.errorMessage).mockReturnValue("Token 余额不足");

      render(StoryJobForm);

      await waitFor(() => {
        expect(screen.getByText(/启动 17-task 流程/)).toBeInTheDocument();
      });

      const startButton = screen.getByText(/启动 17-task 流程/);
      await fireEvent.click(startButton);

      await waitFor(() => {
        expect(screen.getByText("Token 余额不足")).toBeInTheDocument();
      }, { timeout: 3000 });
    });
  });

  describe("任务完成回调", () => {
    it("应该在任务完成时调用回调函数", async () => {
      const onCompleted = vi.fn();
      const mockPreview = { episodes: 6, minutes_per_episode: 2 };
      const mockSnapshot = {
        run_id: "run_test456",
        status: "running",
        progress: { completed: 2, total: 17 },
      };
      const mockCompletedSnapshot = {
        run_id: "run_test456",
        status: "completed",
        progress: { completed: 17, total: 17 },
      };

      vi.mocked(api.desktopApi.validateStoryJob).mockResolvedValue(mockPreview);
      vi.mocked(api.desktopApi.startRun).mockResolvedValue(mockSnapshot);
      vi.mocked(api.desktopApi.syncRun).mockResolvedValue(mockCompletedSnapshot);

      vi.useFakeTimers();

      render(StoryJobForm, { props: { oncompleted: onCompleted } });

      await waitFor(() => {
        expect(screen.getByText(/启动 17-task 流程/)).toBeInTheDocument();
      });

      const startButton = screen.getByText(/启动 17-task 流程/);
      await fireEvent.click(startButton);

      // 等待定时器触发 sync
      await vi.advanceTimersByTimeAsync(1000);

      await waitFor(() => {
        expect(onCompleted).toHaveBeenCalledWith("run_test456");
      });

      vi.useRealTimers();
    });
  });
});
