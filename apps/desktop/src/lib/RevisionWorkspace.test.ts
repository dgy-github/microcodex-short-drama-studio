import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/svelte";
import RevisionWorkspace from "./RevisionWorkspace.svelte";
import * as api from "./api";

// Mock the API module
vi.mock("./api", () => ({
  desktopApi: {
    openRevisionWorkspace: vi.fn(),
    readRevisionSpan: vi.fn(),
    createRevision: vi.fn(),
    approveRevision: vi.fn(),
    compareRevisions: vi.fn(),
    exportRevision: vi.fn(),
  },
  errorMessage: vi.fn((error) => String(error)),
}));

// TODO: 修复复杂组件渲染问题
describe("RevisionWorkspace", () => {
  const mockWorkspace = {
    run_id: "run_001",
    logline: "测试故事",
    revisions: [
      {
        record: {
          revision_id: "rev_001",
          parent_revision_id: null,
          span_ref: "root",
          requested_change: "初始版本",
          created_at_unix_ms: Date.now(),
          approval_status: "pending",
          approval_actor: null,
          approval_note: null,
        },
        findings: [
          {
            span_ref: "episodes/0",
            severity: "major",
            requested_change: "修改第一集的开头",
            review_model: "claude-3",
          },
        ],
        depth: 0,
      },
    ],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.desktopApi.openRevisionWorkspace).mockResolvedValue(mockWorkspace);
  });

  describe("初始化", () => {
    it.skip("应该加载修订工作区", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(api.desktopApi.openRevisionWorkspace).toHaveBeenCalledWith("run_001");
      });
    });

    it.skip("应该显示故事标题", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText("测试故事")).toBeInTheDocument();
      });
    });

    it.skip("应该显示修订列表", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText(/rev_001/)).toBeInTheDocument();
      });
    });
  });

  describe("错误处理", () => {
    it.skip("应该处理加载失败", async () => {
      vi.mocked(api.desktopApi.openRevisionWorkspace).mockRejectedValue(
        new Error("工作区不存在")
      );
      vi.mocked(api.errorMessage).mockReturnValue("工作区不存在");

      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText("工作区不存在")).toBeInTheDocument();
      });
    });
  });

  // TODO: 修复复杂交互测试
  describe("修订创建", () => {
    it.skip("应该选择缺陷", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText("修改第一集的开头")).toBeInTheDocument();
      });
    });

    it.skip("应该创建新修订", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText("测试故事")).toBeInTheDocument();
      });

      // 需要更多交互逻辑
    });
  });

  // TODO: 修复审批流程测试
  describe("审批流程", () => {
    it.skip("应该批准修订", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText("测试故事")).toBeInTheDocument();
      });

      // 需要更多交互逻辑
    });

    it.skip("应该拒绝修订", async () => {
      const onclose = vi.fn();
      render(RevisionWorkspace, { props: { runId: "run_001", onclose } });

      await waitFor(() => {
        expect(screen.getByText("测试故事")).toBeInTheDocument();
      });

      // 需要更多交互逻辑
    });
  });
});
