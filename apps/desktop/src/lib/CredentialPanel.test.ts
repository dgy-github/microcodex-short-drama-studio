import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/svelte";
import CredentialPanel from "./CredentialPanel.svelte";
import * as api from "./api";

// Mock the API module
vi.mock("./api", () => ({
  desktopApi: {
    credentialStatus: vi.fn(),
    providerRoute: vi.fn(),
    credentialAudit: vi.fn(),
    saveProviderRoute: vi.fn(),
    storeCredential: vi.fn(),
    deleteCredential: vi.fn(),
    checkProviderHealth: vi.fn(),
    runProviderSoak: vi.fn(),
  },
  errorMessage: vi.fn((error) => String(error)),
}));

describe("CredentialPanel", () => {
  const mockDeepseekStatus = {
    provider: "deepseek",
    configured: true,
  };

  const mockAliyunStatus = {
    provider: "aliyun_bailian",
    configured: false,
  };

  const mockDeepseekRoute = {
    schema: "desktop-provider-route/v1",
    provider: "deepseek",
    profile: "default",
    endpoint: "https://api.deepseek.com/chat/completions",
    model: "deepseek-chat",
    thinking_disabled: false,
    source: "user",
    record_id: "route_001",
    updated_at_unix_ms: Date.now(),
  };

  const mockAliyunRoute = {
    schema: "desktop-provider-route/v1",
    provider: "aliyun_bailian",
    profile: "default",
    endpoint: "",
    model: "",
    thinking_disabled: true,
    source: "default",
    record_id: null,
    updated_at_unix_ms: null,
  };

  const mockAuditEvents = [
    {
      sequence: 1,
      provider: "deepseek",
      action: "stored",
      timestamp_unix_ms: Date.now(),
    },
  ];

  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(api.desktopApi.credentialStatus).mockImplementation((provider) => {
      if (provider === "deepseek") return Promise.resolve(mockDeepseekStatus);
      return Promise.resolve(mockAliyunStatus);
    });
    vi.mocked(api.desktopApi.providerRoute).mockImplementation((provider) => {
      if (provider === "deepseek") return Promise.resolve(mockDeepseekRoute);
      return Promise.resolve(mockAliyunRoute);
    });
    vi.mocked(api.desktopApi.credentialAudit).mockResolvedValue(mockAuditEvents);
  });

  describe("初始化", () => {
    it("应该显示标题和说明", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("本机凭据保险箱")).toBeInTheDocument();
        expect(screen.getByText("Windows Credential Manager")).toBeInTheDocument();
      });
    });

    it("应该加载提供商状态", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(api.desktopApi.credentialStatus).toHaveBeenCalledWith("deepseek");
        expect(api.desktopApi.credentialStatus).toHaveBeenCalledWith("aliyun_bailian");
      });
    });

    it("应该显示 DeepSeek 提供商", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("DeepSeek")).toBeInTheDocument();
        expect(screen.getByText("故事生成")).toBeInTheDocument();
      });
    });

    it("应该显示阿里云百炼提供商", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("阿里云百炼")).toBeInTheDocument();
        expect(screen.getByText("独立审查")).toBeInTheDocument();
      });
    });
  });

  describe("凭据状态", () => {
    it("应该显示已配置状态", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const configuredElements = screen.getAllByText("凭据已配置");
        expect(configuredElements.length).toBeGreaterThan(0);
      });
    });

    it("应该显示未配置状态", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const unconfiguredElements = screen.getAllByText("凭据未配置");
        expect(unconfiguredElements.length).toBeGreaterThan(0);
      });
    });

    it("应该显示路由来源", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("自定义路由")).toBeInTheDocument();
        expect(screen.getByText("默认路由")).toBeInTheDocument();
      });
    });
  });

  // TODO: 修复 Svelte 5 双向绑定在测试环境的问题
  describe("路由配置", () => {
    it("应该显示已保存的路由配置", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const endpointInputs = screen.getAllByPlaceholderText(/https/);
        expect(endpointInputs[0]).toHaveValue("https://api.deepseek.com/chat/completions");
      });
    });

    it.skip("应该允许编辑 endpoint", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const endpointInputs = screen.getAllByPlaceholderText(/https/);
        expect(endpointInputs[1]).toBeInTheDocument();
      });

      const endpointInput = screen.getAllByPlaceholderText(/https/)[1] as HTMLInputElement;
      await fireEvent.input(endpointInput, {
        target: { value: "https://custom.api.com/v1/chat" },
      });

      expect(endpointInput.value).toBe("https://custom.api.com/v1/chat");
    });

    it.skip("应该允许编辑 model", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const modelInputs = screen.getAllByPlaceholderText("模型 ID");
        expect(modelInputs.length).toBeGreaterThan(0);
      });

      const modelInput = screen.getAllByPlaceholderText("模型 ID")[1] as HTMLInputElement;
      await fireEvent.input(modelInput, {
        target: { value: "custom-model-v1" },
      });

      expect(modelInput.value).toBe("custom-model-v1");
    });

    it.skip("应该保存路由配置", async () => {
      const updatedRoute = { ...mockAliyunRoute, source: "user" };
      vi.mocked(api.desktopApi.saveProviderRoute).mockResolvedValue(updatedRoute);

      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getAllByText("保存地址").length).toBeGreaterThan(0);
      });

      // 输入 endpoint 和 model
      const endpointInputs = screen.getAllByPlaceholderText(/https/);
      const modelInputs = screen.getAllByPlaceholderText("模型 ID");

      await fireEvent.input(endpointInputs[1], {
        target: { value: "https://test.api.com" },
      });
      await fireEvent.input(modelInputs[1], {
        target: { value: "test-model" },
      });

      const saveButtons = screen.getAllByText("保存地址");
      await fireEvent.click(saveButtons[1]);

      await waitFor(() => {
        expect(api.desktopApi.saveProviderRoute).toHaveBeenCalledWith(
          "aliyun_bailian",
          "https://test.api.com",
          "test-model"
        );
      });
    });
  });

  describe("凭据管理", () => {
    it("应该允许输入 API Key", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const secretInputs = screen.getAllByPlaceholderText("粘贴新的 API Key");
        expect(secretInputs.length).toBe(2);
      });

      const secretInput = screen.getAllByPlaceholderText("粘贴新的 API Key")[0] as HTMLInputElement;
      await fireEvent.input(secretInput, {
        target: { value: "sk-test123456" },
      });

      expect(secretInput.value).toBe("sk-test123456");
    });

    it("应该保存凭据", async () => {
      vi.mocked(api.desktopApi.storeCredential).mockResolvedValue({
        provider: "deepseek",
        configured: true,
      });
      vi.mocked(api.desktopApi.credentialAudit).mockResolvedValue([
        ...mockAuditEvents,
        {
          sequence: 2,
          provider: "deepseek",
          action: "stored",
          timestamp_unix_ms: Date.now(),
        },
      ]);

      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getAllByText("保存凭据").length).toBeGreaterThan(0);
      });

      // 输入 API Key
      const secretInputs = screen.getAllByPlaceholderText("粘贴新的 API Key");
      await fireEvent.input(secretInputs[0], {
        target: { value: "sk-new-key" },
      });

      // 点击保存
      const saveButtons = screen.getAllByText("保存凭据");
      await fireEvent.click(saveButtons[0]);

      await waitFor(() => {
        expect(api.desktopApi.storeCredential).toHaveBeenCalledWith("deepseek", "sk-new-key");
      });
    });

    it("应该在保存后清空输入框", async () => {
      vi.mocked(api.desktopApi.storeCredential).mockResolvedValue({
        provider: "deepseek",
        configured: true,
      });

      render(CredentialPanel);

      await waitFor(() => {
        const secretInputs = screen.getAllByPlaceholderText("粘贴新的 API Key");
        expect(secretInputs.length).toBeGreaterThan(0);
      });

      const secretInput = screen.getAllByPlaceholderText("粘贴新的 API Key")[0] as HTMLInputElement;
      await fireEvent.input(secretInput, {
        target: { value: "sk-test-key" },
      });

      const saveButtons = screen.getAllByText("保存凭据");
      await fireEvent.click(saveButtons[0]);

      await waitFor(() => {
        expect(secretInput.value).toBe("");
      });
    });

    it("应该删除凭据", async () => {
      vi.mocked(api.desktopApi.deleteCredential).mockResolvedValue({
        provider: "deepseek",
        configured: false,
      });

      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getAllByText("删除凭据").length).toBeGreaterThan(0);
      });

      const deleteButtons = screen.getAllByText("删除凭据");
      await fireEvent.click(deleteButtons[0]);

      await waitFor(() => {
        expect(api.desktopApi.deleteCredential).toHaveBeenCalledWith("deepseek");
      });
    });
  });

  describe("健康检查", () => {
    it("应该执行健康检查", async () => {
      vi.mocked(api.desktopApi.checkProviderHealth).mockResolvedValue({
        provider: "deepseek",
        model: "deepseek-chat",
        status: "healthy",
      });

      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getAllByText("健康检查").length).toBeGreaterThan(0);
      });

      const healthButtons = screen.getAllByText("健康检查");
      await fireEvent.click(healthButtons[0]);

      await waitFor(() => {
        expect(api.desktopApi.checkProviderHealth).toHaveBeenCalledWith("deepseek");
        expect(screen.getByText(/deepseek-chat 连接正常/)).toBeInTheDocument();
      });
    });

    it("应该在未配置时禁用健康检查按钮", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const healthButtons = screen.getAllByText("健康检查");
        // 阿里云未配置，应该禁用
        expect(healthButtons[1]).toBeDisabled();
      });
    });
  });

  // TODO: 修复 Svelte 5 状态更新和按钮禁用逻辑测试问题
  describe("稳定性检查", () => {
    it.skip("应该显示稳定性检查设置", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("双供应商稳定性检查")).toBeInTheDocument();
        expect(screen.getByText("运行稳定性检查")).toBeInTheDocument();
      });
    });

    it.skip("应该允许设置迭代次数", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        const iterationInput = screen.getByLabelText(/每供应商轮数/) as HTMLInputElement;
        expect(iterationInput).toBeInTheDocument();
      });

      const iterationInput = screen.getByLabelText(/每供应商轮数/) as HTMLInputElement;
      await fireEvent.input(iterationInput, { target: { value: "10" } });

      expect(iterationInput.value).toBe("10");
    });

    it.skip("应该运行稳定性检查", async () => {
      const mockSoakResult = {
        status: "ready",
        iterations_per_provider: 5,
        providers: [
          {
            provider: "deepseek",
            model: "deepseek-chat",
            status: "ready",
            successful_requests: 5,
            failed_requests: 0,
            min_latency_ms: 100,
            average_latency_ms: 150,
            max_latency_ms: 200,
          },
          {
            provider: "aliyun_bailian",
            model: "qwen-max",
            status: "ready",
            successful_requests: 5,
            failed_requests: 0,
            min_latency_ms: 120,
            average_latency_ms: 160,
            max_latency_ms: 210,
          },
        ],
      };

      vi.mocked(api.desktopApi.runProviderSoak).mockResolvedValue(mockSoakResult);

      // 需要两个提供商都配置
      vi.mocked(api.desktopApi.credentialStatus).mockResolvedValue({
        provider: "deepseek",
        configured: true,
      });

      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("运行稳定性检查")).toBeInTheDocument();
      });

      const soakButton = screen.getByText("运行稳定性检查");
      // 需要等待两个提供商都标记为已配置
      await waitFor(() => {
        expect(soakButton).not.toBeDisabled();
      }, { timeout: 5000 });

      await fireEvent.click(soakButton);

      await waitFor(() => {
        expect(api.desktopApi.runProviderSoak).toHaveBeenCalledWith(5);
      }, { timeout: 5000 });
    });

    it.skip("应该显示稳定性检查结果", async () => {
      const mockSoakResult = {
        status: "ready",
        iterations_per_provider: 5,
        providers: [
          {
            provider: "deepseek",
            model: "deepseek-chat",
            status: "ready",
            successful_requests: 5,
            failed_requests: 0,
            min_latency_ms: 100,
            average_latency_ms: 150,
            max_latency_ms: 200,
          },
        ],
      };

      vi.mocked(api.desktopApi.runProviderSoak).mockResolvedValue(mockSoakResult);
      vi.mocked(api.desktopApi.credentialStatus).mockResolvedValue({
        provider: "deepseek",
        configured: true,
      });

      render(CredentialPanel);

      await waitFor(() => {
        const soakButton = screen.getByText("运行稳定性检查");
        expect(soakButton).not.toBeDisabled();
      }, { timeout: 5000 });

      const soakButton = screen.getByText("运行稳定性检查");
      await fireEvent.click(soakButton);

      await waitFor(() => {
        expect(screen.getByText(/5\/5 成功/)).toBeInTheDocument();
        expect(screen.getByText(/100\/150\/200 ms/)).toBeInTheDocument();
      }, { timeout: 5000 });
    });
  });

  describe("审计日志", () => {
    it("应该显示最近的审计事件", async () => {
      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText(/最近审计 #1/)).toBeInTheDocument();
        expect(screen.getByText(/deepseek/)).toBeInTheDocument();
        expect(screen.getByText(/stored/)).toBeInTheDocument();
      });
    });
  });

  describe("错误处理", () => {
    it("应该处理初始化错误", async () => {
      vi.mocked(api.desktopApi.credentialStatus).mockRejectedValue(
        new Error("网络错误")
      );
      vi.mocked(api.errorMessage).mockReturnValue("网络错误");

      render(CredentialPanel);

      await waitFor(() => {
        expect(screen.getByText("网络错误")).toBeInTheDocument();
      });
    });

    it("应该处理保存凭据错误", async () => {
      vi.mocked(api.desktopApi.storeCredential).mockRejectedValue(
        new Error("凭据格式无效")
      );
      vi.mocked(api.errorMessage).mockReturnValue("凭据格式无效");

      render(CredentialPanel);

      await waitFor(() => {
        const secretInputs = screen.getAllByPlaceholderText("粘贴新的 API Key");
        expect(secretInputs.length).toBeGreaterThan(0);
      });

      const secretInput = screen.getAllByPlaceholderText("粘贴新的 API Key")[0];
      await fireEvent.input(secretInput, { target: { value: "invalid" } });

      const saveButtons = screen.getAllByText("保存凭据");
      await fireEvent.click(saveButtons[0]);

      await waitFor(() => {
        expect(screen.getByText("凭据格式无效")).toBeInTheDocument();
      });
    });
  });

  describe("按钮状态", () => {
    it("应该在操作进行时禁用所有按钮", async () => {
      vi.mocked(api.desktopApi.storeCredential).mockImplementation(
        () => new Promise((resolve) => setTimeout(resolve, 1000))
      );

      render(CredentialPanel);

      await waitFor(() => {
        const secretInputs = screen.getAllByPlaceholderText("粘贴新的 API Key");
        expect(secretInputs.length).toBeGreaterThan(0);
      });

      const secretInput = screen.getAllByPlaceholderText("粘贴新的 API Key")[0];
      await fireEvent.input(secretInput, { target: { value: "sk-test" } });

      const saveButtons = screen.getAllByText("保存凭据");
      await fireEvent.click(saveButtons[0]);

      // 其他按钮应该被禁用
      const healthButtons = screen.getAllByText("健康检查");
      expect(healthButtons[0]).toBeDisabled();
    });
  });
});
