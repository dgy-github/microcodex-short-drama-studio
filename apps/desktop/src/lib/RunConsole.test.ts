import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import RunConsole from "./RunConsole.svelte";
import type { RunSnapshot } from "./types";

describe("RunConsole", () => {
  const createMockSnapshot = (overrides?: Partial<RunSnapshot>): RunSnapshot => ({
    run_id: "run_test123456789",
    status: "running",
    tasks_completed: 5,
    tasks_total: 17,
    reviews_completed: 2,
    approvals_pending: 1,
    budget: {
      max_tokens: 180000,
      max_cny_fen: 1200,
      deadline_seconds: 900,
      consumed_tokens: 50000,
      consumed_cny_fen: 400,
    },
    last_event_id: "evt_001",
    events: [],
    error: null,
    ...overrides,
  });

  beforeEach(() => {
    vi.clearAllMocks();
  });

  describe("基本渲染", () => {
    it("应该显示运行ID和状态", () => {
      const snapshot = createMockSnapshot();
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText(/23456789/)).toBeInTheDocument();
      expect(screen.getByText("RUNNING")).toBeInTheDocument();
    });

    it("应该显示进度条", () => {
      const snapshot = createMockSnapshot({ tasks_completed: 10, tasks_total: 17 });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const progressBar = document.querySelector(".progress-track span") as HTMLElement;
      expect(progressBar).toBeInTheDocument();
      // 10/17 ≈ 59%
      expect(progressBar.style.width).toBe("59%");
    });

    it("应该显示任务统计", () => {
      const snapshot = createMockSnapshot({
        tasks_completed: 8,
        reviews_completed: 3,
        approvals_pending: 2,
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("8/17")).toBeInTheDocument();
      expect(screen.getByText("3/5")).toBeInTheDocument();
      expect(screen.getByText("2")).toBeInTheDocument();
    });

    it("应该显示 Token 消耗", () => {
      const snapshot = createMockSnapshot({
        budget: {
          max_tokens: 180000,
          max_cny_fen: 1200,
          deadline_seconds: 900,
          consumed_tokens: 75000,
          consumed_cny_fen: 600,
        },
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("75,000")).toBeInTheDocument();
      expect(screen.getByText(/180,000/)).toBeInTheDocument();
    });
  });

  describe("费用显示", () => {
    it("应该在费用为 null 时显示提示", () => {
      const snapshot = createMockSnapshot({
        budget: {
          max_tokens: 180000,
          max_cny_fen: 1200,
          deadline_seconds: 900,
          consumed_tokens: 50000,
          consumed_cny_fen: null,
        },
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText(/费用消耗暂不可计算/)).toBeInTheDocument();
      expect(screen.getByText(/¥12.00/)).toBeInTheDocument();
    });

    it("应该在费用可用时不显示提示", () => {
      const snapshot = createMockSnapshot({
        budget: {
          max_tokens: 180000,
          max_cny_fen: 1200,
          deadline_seconds: 900,
          consumed_tokens: 50000,
          consumed_cny_fen: 800,
        },
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.queryByText(/费用消耗暂不可计算/)).not.toBeInTheDocument();
    });
  });

  describe("错误显示", () => {
    it("应该显示错误信息", () => {
      const snapshot = createMockSnapshot({
        error: "Token 余额不足",
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("Token 余额不足")).toBeInTheDocument();
    });

    it("应该在无错误时不显示错误", () => {
      const snapshot = createMockSnapshot({ error: null });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const errorElements = document.querySelectorAll(".error");
      expect(errorElements.length).toBe(0);
    });
  });

  describe("事件列表", () => {
    it("应该显示空事件列表提示", () => {
      const snapshot = createMockSnapshot({
        events: [],
        last_event_id: "evt_123",
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText(/等待新的持久事件/)).toBeInTheDocument();
      expect(screen.getByText(/evt_123/)).toBeInTheDocument();
    });

    it("应该显示事件列表（倒序）", () => {
      const snapshot = createMockSnapshot({
        events: [
          { event_id: "evt_1", seq: 1, event_type: "task_started", task_id: "task_001" },
          { event_id: "evt_2", seq: 2, event_type: "task_completed", task_id: "task_001" },
          { event_id: "evt_3", seq: 3, event_type: "review_started", task_id: "review_001" },
        ],
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("#1")).toBeInTheDocument();
      expect(screen.getByText("task_started")).toBeInTheDocument();
      expect(screen.getAllByText("task_001").length).toBeGreaterThan(0);
      expect(screen.getByText("#3")).toBeInTheDocument();
      expect(screen.getByText("review_started")).toBeInTheDocument();
    });

    it("应该显示 run 级别的事件", () => {
      const snapshot = createMockSnapshot({
        events: [
          { event_id: "evt_1", seq: 1, event_type: "run_started", task_id: null },
        ],
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("#1")).toBeInTheDocument();
      expect(screen.getByText("run_started")).toBeInTheDocument();
      expect(screen.getByText("run")).toBeInTheDocument();
    });
  });

  describe("取消按钮", () => {
    it("应该在运行时显示中止按钮", () => {
      const snapshot = createMockSnapshot({ status: "running" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const cancelButton = screen.getByText("中止运行");
      expect(cancelButton).toBeInTheDocument();
      expect(cancelButton).not.toBeDisabled();
    });

    it("应该调用 oncancel 回调", async () => {
      const oncancel = vi.fn();
      const snapshot = createMockSnapshot({ status: "running" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel } });

      const cancelButton = screen.getByText("中止运行");
      await fireEvent.click(cancelButton);

      expect(oncancel).toHaveBeenCalledTimes(1);
    });

    it("应该在取消中时显示正在取消", () => {
      const snapshot = createMockSnapshot({ status: "running" });
      render(RunConsole, { props: { snapshot, cancelling: true, oncancel: vi.fn() } });

      const cancelButton = screen.getByText("正在取消…");
      expect(cancelButton).toBeInTheDocument();
      expect(cancelButton).toBeDisabled();
    });

    it("应该在已完成时禁用按钮", () => {
      const snapshot = createMockSnapshot({ status: "completed" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const cancelButton = screen.getByText("中止运行");
      expect(cancelButton).toBeDisabled();
    });

    it("应该在失败时禁用按钮", () => {
      const snapshot = createMockSnapshot({ status: "failed" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const cancelButton = screen.getByText("中止运行");
      expect(cancelButton).toBeDisabled();
    });

    it("应该在已取消时禁用按钮", () => {
      const snapshot = createMockSnapshot({ status: "cancelled" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const cancelButton = screen.getByText("中止运行");
      expect(cancelButton).toBeDisabled();
    });
  });

  describe("状态显示", () => {
    it("应该显示 accepted 状态", () => {
      const snapshot = createMockSnapshot({ status: "accepted" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("ACCEPTED")).toBeInTheDocument();
    });

    it("应该显示 completed 状态", () => {
      const snapshot = createMockSnapshot({ status: "completed", tasks_completed: 17 });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("COMPLETED")).toBeInTheDocument();
    });

    it("应该显示 failed 状态", () => {
      const snapshot = createMockSnapshot({ status: "failed", error: "执行失败" });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("FAILED")).toBeInTheDocument();
    });
  });

  describe("进度计算", () => {
    it("应该计算 0% 进度", () => {
      const snapshot = createMockSnapshot({ tasks_completed: 0, tasks_total: 17 });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const progressBar = document.querySelector(".progress-track span") as HTMLElement;
      expect(progressBar.style.width).toBe("0%");
    });

    it("应该计算 100% 进度", () => {
      const snapshot = createMockSnapshot({ tasks_completed: 17, tasks_total: 17 });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const progressBar = document.querySelector(".progress-track span") as HTMLElement;
      expect(progressBar.style.width).toBe("100%");
    });

    it("应该计算 50% 进度", () => {
      const snapshot = createMockSnapshot({ tasks_completed: 8, tasks_total: 16 });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      const progressBar = document.querySelector(".progress-track span") as HTMLElement;
      expect(progressBar.style.width).toBe("50%");
    });
  });

  describe("边界情况", () => {
    it("应该处理 0 Token 消耗", () => {
      const snapshot = createMockSnapshot({
        budget: {
          max_tokens: 180000,
          max_cny_fen: 1200,
          deadline_seconds: 900,
          consumed_tokens: 0,
          consumed_cny_fen: 0,
        },
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      expect(screen.getByText("0")).toBeInTheDocument();
    });

    it("应该处理大量事件", () => {
      const events = Array.from({ length: 50 }, (_, i) => ({
        event_id: `evt_${i}`,
        seq: i + 1,
        event_type: "task_event",
        task_id: `task_${i}`,
      }));

      const snapshot = createMockSnapshot({ events });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      // 应该渲染所有事件
      expect(screen.getByText("#1")).toBeInTheDocument();
      expect(screen.getByText("#50")).toBeInTheDocument();
    });

    it("应该处理非常长的 run_id", () => {
      const snapshot = createMockSnapshot({
        run_id: "run_verylongrunidwithmanycharacters123456789",
      });
      render(RunConsole, { props: { snapshot, cancelling: false, oncancel: vi.fn() } });

      // 应该只显示最后8个字符
      expect(screen.getByText(/56789/)).toBeInTheDocument();
    });
  });
});
