import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor, fireEvent } from "@testing-library/svelte";
import RevisionWorkspace from "./RevisionWorkspace.svelte";
import * as api from "./api";
import type { RevisionSummary, RevisionWorkspace as RevisionWorkspaceType } from "./types";

vi.mock("./api", () => ({
  desktopApi: {
    openRevisionWorkspace: vi.fn(),
    readRevisionSpan: vi.fn(),
    createRevision: vi.fn(),
    approveRevision: vi.fn(),
    compareRevisions: vi.fn(),
    rollbackRevision: vi.fn(),
    exportRevision: vi.fn(),
  },
  errorMessage: vi.fn((error) => String(error)),
}));

describe("RevisionWorkspace", () => {
  const origin: RevisionSummary = {
    schema: "desktop-revision-summary/v1",
    record: {
      schema: "story-revision-record/v1",
      revision_id: "rev_001",
      job_id: "job_001",
      package_id: "pkg_001",
      supersedes_package_id: null,
      kind: "origin",
      round: 0,
      source_run_id: "run_001",
      target_span: null,
      requested_change: "初始版本",
      content_sha256: "sha256_001",
      created_at_unix_ms: 1,
      node_correspondence_count: 0,
    },
    approval: null,
  };

  const revised: RevisionSummary = {
    schema: "desktop-revision-summary/v1",
    record: {
      ...origin.record,
      revision_id: "rev_002",
      supersedes_package_id: "pkg_001",
      package_id: "pkg_002",
      kind: "targeted",
      round: 1,
      target_span: "episodes/0",
      requested_change: "修改第一集的开头",
      content_sha256: "sha256_002",
      created_at_unix_ms: 2,
      node_correspondence_count: 1,
    },
    approval: null,
  };

  const workspace: RevisionWorkspaceType = {
    run_id: "run_001",
    job_id: "job_001",
    revisions: [origin],
    findings: [{
      defect_id: "defect_001",
      span_ref: "episodes/0",
      severity: "major",
      evidence: "第一集开头缺少明确冲突",
      requested_change: "修改第一集的开头",
    }],
  };

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.desktopApi.openRevisionWorkspace).mockResolvedValue(workspace);
    vi.mocked(api.desktopApi.readRevisionSpan).mockResolvedValue({ node_id: "episode_1", title: "旧标题" });
    vi.mocked(api.desktopApi.createRevision).mockResolvedValue(revised);
  });

  it("应该加载修订工作区", async () => {
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    await waitFor(() => expect(api.desktopApi.openRevisionWorkspace).toHaveBeenCalledWith("run_001"));
  });

  it("应该显示工作区标题与当前版本", async () => {
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    expect(await screen.findByText("定向修订与审批")).toBeInTheDocument();
    expect(await screen.findByText("rev_001")).toBeInTheDocument();
  });

  it("应该显示修订列表", async () => {
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    expect(await screen.findByText("R0 · origin")).toBeInTheDocument();
  });

  it("应该处理加载失败", async () => {
    vi.mocked(api.desktopApi.openRevisionWorkspace).mockRejectedValue(new Error("工作区不存在"));
    vi.mocked(api.errorMessage).mockReturnValue("工作区不存在");
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    expect(await screen.findByText("工作区不存在")).toBeInTheDocument();
  });

  it("应该选择缺陷并读取目标节点", async () => {
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    await fireEvent.click(await screen.findByText("修改第一集的开头"));
    await waitFor(() => expect(api.desktopApi.readRevisionSpan).toHaveBeenCalledWith("rev_001", "episodes/0"));
    expect((screen.getByLabelText(/替换完整节点 JSON/) as HTMLTextAreaElement).value).toContain("episode_1");
  });

  it("应该创建新修订", async () => {
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    await fireEvent.click(await screen.findByText("修改第一集的开头"));
    const editor = await screen.findByLabelText(/替换完整节点 JSON/);
    await waitFor(() => expect((editor as HTMLTextAreaElement).value).toContain("episode_1"));
    await fireEvent.click(screen.getByText("创建不可变修订"));
    await waitFor(() => expect(api.desktopApi.createRevision).toHaveBeenCalledWith(
      "rev_001",
      "episodes/0",
      { node_id: "episode_1", title: "旧标题" },
      "修改第一集的开头",
    ));
    expect(await screen.findByText("新修订已作为不可变版本保存。")).toBeInTheDocument();
  });

  it("应该批准修订", async () => {
    const approved: RevisionSummary = {
      ...origin,
      approval: {
        schema: "story-approval-event/v1",
        approval_id: "approval_1",
        revision_id: "rev_001",
        decision: "approved",
        actor: "operator",
        note: "",
        occurred_at_unix_ms: 3,
      },
    };
    vi.mocked(api.desktopApi.approveRevision).mockResolvedValue(approved);
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    await fireEvent.click(await screen.findByText("批准"));
    await waitFor(() => expect(api.desktopApi.approveRevision).toHaveBeenCalledWith("rev_001", "approved", "operator", ""));
    expect(await screen.findByText("版本已批准，可导出。")).toBeInTheDocument();
  });

  it("应该拒绝修订", async () => {
    const rejected: RevisionSummary = {
      ...origin,
      approval: {
        schema: "story-approval-event/v1",
        approval_id: "approval_2",
        revision_id: "rev_001",
        decision: "rejected",
        actor: "operator",
        note: "",
        occurred_at_unix_ms: 3,
      },
    };
    vi.mocked(api.desktopApi.approveRevision).mockResolvedValue(rejected);
    render(RevisionWorkspace, { props: { runId: "run_001", onclose: vi.fn() } });
    await fireEvent.click(await screen.findByText("拒绝"));
    await waitFor(() => expect(api.desktopApi.approveRevision).toHaveBeenCalledWith("rev_001", "rejected", "operator", ""));
    expect(await screen.findByText("版本已拒绝。")).toBeInTheDocument();
  });
});
