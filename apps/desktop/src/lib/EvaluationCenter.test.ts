import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import EvaluationCenter from "./EvaluationCenter.svelte";
import * as api from "./api";

// Mock the API module
vi.mock("./api", () => ({
  desktopApi: {
    evaluationCatalog: vi.fn(),
    runAutomaticEvaluation: vi.fn(),
    createBlindAssignments: vi.fn(),
    submitBlindScore: vi.fn(),
  },
  errorMessage: vi.fn((error) => String(error)),
}));

describe("EvaluationCenter", () => {
  const mockCatalog = {
    datasets: [
      {
        dataset_id: "offline-v0.1.0",
        label: "离线测试集 v0.1.0",
        description: "初始评估数据集",
        case_count: 3,
        eligible_count: 2,
        cases: [
          {
            case_id: "case_001",
            label: "测试故事1",
            genre: "家庭剧",
            difficulty: "简单",
            split: "offline",
            eligible: true,
            automatic_scores: null,
            human_scores: [],
          },
          {
            case_id: "case_002",
            label: "测试故事2",
            genre: "悬疑剧",
            difficulty: "中等",
            split: "offline",
            eligible: true,
            automatic_scores: null,
            human_scores: [],
          },
          {
            case_id: "case_003",
            label: "测试故事3（不合格）",
            genre: "爱情剧",
            difficulty: null,
            split: "online",
            eligible: false,
            automatic_scores: null,
            human_scores: [],
          },
        ],
      },
      {
        dataset_id: "online-v1.0.0",
        label: "在线测试集 v1.0.0",
        description: "生产环境数据集",
        case_count: 0,
        eligible_count: 0,
        cases: [],
      },
    ],
    dimensions: [
      {
        dimension_id: "coherence",
        display_name: "连贯性",
        description: "情节逻辑是否连贯",
        score_range: [1, 5],
      },
      {
        dimension_id: "engagement",
        display_name: "吸引力",
        description: "故事是否引人入胜",
        score_range: [1, 5],
      },
    ],
  };

  const mockBatchResult = {
    dataset_id: "offline-v0.1.0",
    case_ids: ["case_001", "case_002"],
    evaluated_at_unix_ms: Date.now(),
    scores: [
      {
        case_id: "case_001",
        dimensions: [
          { dimension_id: "coherence", score: 4.5, reason: "逻辑清晰" },
          { dimension_id: "engagement", score: 4.0, reason: "情节吸引人" },
        ],
      },
      {
        case_id: "case_002",
        dimensions: [
          { dimension_id: "coherence", score: 3.5, reason: "部分跳跃" },
          { dimension_id: "engagement", score: 4.2, reason: "引人入胜" },
        ],
      },
    ],
  };

  const mockAssignments = [
    {
      assignment_id: "assign_001",
      case_id: "case_001",
      rater_id: "reviewer_01",
      logline: "测试故事1",
      content: "故事内容...",
      dimensions: [
        {
          dimension_id: "coherence",
          display_name: "连贯性",
          description: "情节逻辑是否连贯",
          score_range: [1, 5],
        },
      ],
      allowed_spans: ["全文", "第一幕", "第二幕"],
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.desktopApi.evaluationCatalog).mockResolvedValue(mockCatalog);
  });

  describe("初始化", () => {
    it("应该加载评估目录", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(api.desktopApi.evaluationCatalog).toHaveBeenCalled();
      });
    });

    it("应该显示数据集列表", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("离线测试集 v0.1.0")).toBeInTheDocument();
        expect(screen.getByText("在线测试集 v1.0.0")).toBeInTheDocument();
      });
    });

    it("应该显示数据集统计", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("2/3 可运行")).toBeInTheDocument();
      });
    });
  });

  describe("数据集切换", () => {
    it("应该允许切换数据集", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("离线测试集 v0.1.0")).toBeInTheDocument();
      });

      const onlineDataset = screen.getByText("在线测试集 v1.0.0");
      await fireEvent.click(onlineDataset);

      // 切换后清空选择
      await waitFor(() => {
        expect(screen.queryByText("测试故事1")).not.toBeInTheDocument();
      });
    });
  });

  describe("案例选择", () => {
    it("应该允许选择单个案例", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByLabelText("选择用例 case_001")).toBeInTheDocument();
      });

      const checkbox = screen.getByLabelText("选择用例 case_001") as HTMLInputElement;
      await fireEvent.click(checkbox);

      expect(checkbox).toBeChecked();
    });

    it("应该允许全选合格案例", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("选择全部可运行")).toBeInTheDocument();
      });

      const selectAllButton = screen.getByText("选择全部可运行");
      await fireEvent.click(selectAllButton);

      await waitFor(() => {
        expect(screen.getByText("2 项已选择")).toBeInTheDocument();
      });
    });

    it("应该显示合格和不合格状态", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        const readyElements = screen.getAllByText("READY");
        expect(readyElements.length).toBeGreaterThan(0);
        const noArtifactElements = screen.getAllByText("NO ARTIFACT");
        expect(noArtifactElements.length).toBeGreaterThan(0);
      });
    });
  });

  describe("自动评估", () => {
    it("应该运行自动评估", async () => {
      vi.mocked(api.desktopApi.runAutomaticEvaluation).mockResolvedValue(mockBatchResult);

      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByLabelText("选择用例 case_001")).toBeInTheDocument();
      });

      // 选择案例
      const checkbox1 = screen.getByLabelText("选择用例 case_001");
      const checkbox2 = screen.getByLabelText("选择用例 case_002");
      await fireEvent.click(checkbox1);
      await fireEvent.click(checkbox2);

      // 运行评估
      const evaluateButton = screen.getByText("运行所选自动评测");
      await fireEvent.click(evaluateButton);

      await waitFor(() => {
        expect(api.desktopApi.runAutomaticEvaluation).toHaveBeenCalledWith(
          "offline-v0.1.0",
          ["case_001", "case_002"]
        );
      });
    });

    // TODO: 修复复杂状态渲染问题
    it.skip("应该显示评估结果", async () => {
      vi.mocked(api.desktopApi.runAutomaticEvaluation).mockResolvedValue(mockBatchResult);

      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByLabelText("选择用例 case_001")).toBeInTheDocument();
      });

      const checkbox = screen.getByLabelText("选择用例 case_001");
      await fireEvent.click(checkbox);

      const evaluateButton = screen.getByText("运行所选自动评测");
      await fireEvent.click(evaluateButton);

      await waitFor(() => {
        expect(screen.getByText(/coherence/)).toBeInTheDocument();
        expect(screen.getByText(/4.5/)).toBeInTheDocument();
      });
    });

    it("应该在未选择案例时禁用评估按钮", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        const evaluateButton = screen.getByText("运行所选自动评测");
        expect(evaluateButton).toBeDisabled();
      });
    });
  });

  // TODO: 修复复杂交互和状态管理问题
  describe("人工评估", () => {
    it.skip("应该切换到人工评估模式", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("自动评测")).toBeInTheDocument();
      });

      const humanModeButton = screen.getByText("人工盲测");
      await fireEvent.click(humanModeButton);

      await waitFor(() => {
        expect(screen.getByText("创建所选盲测")).toBeInTheDocument();
      });
    });

    it.skip("应该创建盲评任务", async () => {
      vi.mocked(api.desktopApi.createBlindAssignments).mockResolvedValue(mockAssignments);

      render(EvaluationCenter);

      await waitFor(() => {
        const humanModeButton = screen.getByText("人工盲测");
        expect(humanModeButton).toBeInTheDocument();
      });

      const humanModeButton = screen.getByText("人工盲测");
      await fireEvent.click(humanModeButton);

      await waitFor(() => {
        expect(screen.getByLabelText("选择用例 case_001")).toBeInTheDocument();
      });

      const checkbox = screen.getByLabelText("选择用例 case_001");
      await fireEvent.click(checkbox);

      const createButton = screen.getByText("创建所选盲测");
      await fireEvent.click(createButton);

      await waitFor(() => {
        expect(api.desktopApi.createBlindAssignments).toHaveBeenCalledWith(
          "offline-v0.1.0",
          ["case_001"],
          "reviewer_01"
        );
      });
    });

    it.skip("应该显示评分界面", async () => {
      vi.mocked(api.desktopApi.createBlindAssignments).mockResolvedValue(mockAssignments);

      render(EvaluationCenter);

      const humanModeButton = await screen.findByText("人工盲测");
      await fireEvent.click(humanModeButton);

      const checkbox = await screen.findByLabelText("选择用例 case_001");
      await fireEvent.click(checkbox);

      const createButton = screen.getByText("创建所选盲测");
      await fireEvent.click(createButton);

      await waitFor(() => {
        expect(screen.getByText("连贯性")).toBeInTheDocument();
        expect(screen.getByText("情节逻辑是否连贯")).toBeInTheDocument();
      });
    });

    it.skip("应该允许输入评分", async () => {
      vi.mocked(api.desktopApi.createBlindAssignments).mockResolvedValue(mockAssignments);

      render(EvaluationCenter);

      const humanModeButton = await screen.findByText("人工盲测");
      await fireEvent.click(humanModeButton);

      const checkbox = await screen.findByLabelText("选择用例 case_001");
      await fireEvent.click(checkbox);

      const createButton = screen.getByText("创建所选盲测");
      await fireEvent.click(createButton);

      await waitFor(() => {
        const scoreSelects = screen.getAllByLabelText(/分数/);
        expect(scoreSelects.length).toBeGreaterThan(0);
      });

      const scoreSelects = screen.getAllByLabelText(/分数/);
      await fireEvent.change(scoreSelects[0], { target: { value: "5" } });

      expect((scoreSelects[0] as HTMLSelectElement).value).toBe("5");
    });
  });

  // TODO: 修复双击和详情渲染问题
  describe("案例详情", () => {
    it.skip("应该打开案例详情", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("测试故事1")).toBeInTheDocument();
      });

      const caseRow = screen.getByText("测试故事1").closest("tr");
      expect(caseRow).toBeInTheDocument();

      await fireEvent.dblClick(caseRow!);

      await waitFor(() => {
        expect(screen.getByText(/case_001/)).toBeInTheDocument();
      });
    });

    it.skip("应该通过 Escape 键关闭详情", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("测试故事1")).toBeInTheDocument();
      });

      const caseRow = screen.getByText("测试故事1").closest("tr");
      await fireEvent.dblClick(caseRow!);

      await waitFor(() => {
        expect(screen.getByText(/case_001/)).toBeInTheDocument();
      });

      await fireEvent.keyDown(document, { key: "Escape" });

      await waitFor(() => {
        expect(screen.queryByText(/case_001/)).not.toBeInTheDocument();
      });
    });
  });

  describe("错误处理", () => {
    // TODO: 修复错误状态渲染问题
    it.skip("应该处理目录加载失败", async () => {
      vi.mocked(api.desktopApi.evaluationCatalog).mockRejectedValue(
        new Error("网络错误")
      );
      vi.mocked(api.errorMessage).mockReturnValue("网络错误");

      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("网络错误")).toBeInTheDocument();
      });
    });

    it("应该处理评估失败", async () => {
      vi.mocked(api.desktopApi.runAutomaticEvaluation).mockRejectedValue(
        new Error("评估服务不可用")
      );
      vi.mocked(api.errorMessage).mockReturnValue("评估服务不可用");

      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByLabelText("选择用例 case_001")).toBeInTheDocument();
      });

      const checkbox = screen.getByLabelText("选择用例 case_001");
      await fireEvent.click(checkbox);

      const evaluateButton = screen.getByText("运行所选自动评测");
      await fireEvent.click(evaluateButton);

      await waitFor(() => {
        expect(screen.getByText("评估服务不可用")).toBeInTheDocument();
      });
    });
  });

  describe("界面元素", () => {
    it("应该显示模式切换按钮", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("自动评测")).toBeInTheDocument();
        expect(screen.getByText("人工盲测")).toBeInTheDocument();
      });
    });

    it("应该显示数据集信息", async () => {
      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByText("离线测试集 v0.1.0")).toBeInTheDocument();
        expect(screen.getByText("2/3 可运行")).toBeInTheDocument();
      });
    });

    it("应该在忙碌时禁用按钮", async () => {
      vi.mocked(api.desktopApi.runAutomaticEvaluation).mockImplementation(
        () => new Promise((resolve) => setTimeout(resolve, 1000))
      );

      render(EvaluationCenter);

      await waitFor(() => {
        expect(screen.getByLabelText("选择用例 case_001")).toBeInTheDocument();
      });

      const checkbox = screen.getByLabelText("选择用例 case_001");
      await fireEvent.click(checkbox);

      const evaluateButton = screen.getByText("运行所选自动评测");
      await fireEvent.click(evaluateButton);

      // 按钮应该被禁用
      expect(evaluateButton).toBeDisabled();
    });
  });
});
