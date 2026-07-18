import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import { createPreviewBackend } from "./lib/previewBackend";

describe("讯栖多任务工作台", () => {
  beforeEach(() => window.localStorage.removeItem("xunqi.interface-language"));

  it("按来源分组，并可搜索和切换公众号/视频号筛选", async () => {
    const user = userEvent.setup();
    render(<App backend={createPreviewBackend()} />);

    expect(await screen.findByText("捕获任务")).toBeInTheDocument();
    const sidebar = screen.getByRole("complementary", { name: "捕获任务" });
    expect(within(sidebar).getByText("示例科技周报")).toBeInTheDocument();
    expect(within(sidebar).getByText("影像测试频道")).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /公众号 6/ })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /视频号 6/ })).toBeInTheDocument();

    await user.type(screen.getByLabelText("搜索公众号、视频号或标题"), "影像测试");
    expect(within(sidebar).queryByText("示例科技周报")).not.toBeInTheDocument();
    expect(within(sidebar).getByText("影像测试频道")).toBeInTheDocument();

    await user.clear(screen.getByLabelText("搜索公众号、视频号或标题"));
    await user.click(screen.getByRole("tab", { name: /视频号 6/ }));
    expect(screen.queryByRole("button", { name: /本地内容整理工作流/ })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: /公开直链识别演示/ })).toBeInTheDocument();
  });

  it("可按当前公众号或视频号筛选结果一键全选，并在切换筛选时保留选择", async () => {
    const user = userEvent.setup();
    render(<App backend={createPreviewBackend()} />);

    await screen.findByText("捕获任务");
    await user.click(screen.getByRole("tab", { name: /公众号 6/ }));
    await user.click(screen.getByRole("button", { name: "全选公众号 6 项" }));
    expect(screen.getByText("已选 6 项")).toBeInTheDocument();
    expect(screen.getByText("公众号 6")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: /视频号 6/ }));
    expect(screen.getByText("已选 6 项")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "全选视频号 6 项" }));
    expect(screen.getByText("已选 12 项")).toBeInTheDocument();
    expect(screen.getByText("公众号 6 · 视频号 6")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "取消当前视频号 6 项" }));
    expect(screen.getByText("已选 6 项")).toBeInTheDocument();
    expect(screen.getByText("公众号 6")).toBeInTheDocument();
  });

  it("从全部筛选后切换分类时可明确取消全部已选项", async () => {
    const user = userEvent.setup();
    render(<App backend={createPreviewBackend()} />);

    await screen.findByText("捕获任务");
    await user.click(screen.getByRole("button", { name: "全选全部 12 项" }));
    expect(screen.getByText("已选 12 项")).toBeInTheDocument();
    expect(screen.getByText("公众号 6 · 视频号 6")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: /视频号 6/ }));
    expect(screen.getByRole("button", { name: "取消当前视频号 6 项" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "取消全部已选 12 项" }));
    expect(screen.queryByText(/^已选 \d+ 项$/)).not.toBeInTheDocument();
  });

  it("把微信分享链接粘贴到搜索框时改为收取任务而不是过滤列表", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const existing = (await backend.listTasks())[0];
    const submitLinks = vi.fn(async () => ({ tasks: [existing.task], duplicateCount: 1 }));
    backend.submitLinks = submitLinks;
    backend.detectWechatForeground = async () => ({
      isWechatFrontmost: false,
      applicationName: "讯栖",
      bundleIdentifier: "com.xiaofu.xunqi",
      limitation: "",
    });
    render(<App backend={backend} />);

    const search = await screen.findByLabelText("搜索公众号、视频号或标题");
    await user.click(search);
    await user.paste("https://mp.weixin.qq.com/s/public-demo-link?scene=1");

    await waitFor(() => expect(submitLinks).toHaveBeenCalledWith(
      "https://mp.weixin.qq.com/s/public-demo-link?scene=1",
    ));
    expect(search).toHaveValue("");
    expect(screen.queryByText("没有匹配的任务")).not.toBeInTheDocument();
  });

  it("展示 QIDU 品牌题记，并在关于页说明本地与授权边界", async () => {
    const user = userEvent.setup();
    render(<App backend={createPreviewBackend()} />);

    const [header] = await screen.findAllByRole("banner");
    expect(within(header).getByText(/讯栖/)).toHaveTextContent("讯栖 XunQi");
    expect(within(header).getByText("讯来有迹，文止于栖。")).toBeInTheDocument();
    expect(within(header).getByText("A QIDU Utility")).toBeInTheDocument();
    expect(within(header).getByRole("button", { name: "导出日志" })).toBeInTheDocument();

    await user.click(within(header).getByRole("button", { name: "关于" }));
    const dialog = screen.getByRole("dialog", { name: "讯栖 XunQi" });
    expect(within(dialog).getByText("栖 · CHAPTER 01")).toBeInTheDocument();
    expect(within(dialog).getByText(/只接住你主动复制的微信分享链接/)).toBeInTheDocument();
    expect(within(dialog).getByText("明确授权")).toBeInTheDocument();
    expect(within(dialog).getByText("本地诊断日志")).toBeInTheDocument();
    expect(within(dialog).getByText(/不记录 Cookie、聊天记录/)).toBeInTheDocument();
  });

  it("提供中英文切换，并切换主要工作台文案", async () => {
    const user = userEvent.setup();
    render(<App backend={createPreviewBackend()} />);

    const [header] = await screen.findAllByRole("banner");
    await user.click(within(header).getByRole("button", { name: "切换为英文" }));

    expect(within(header).getByRole("button", { name: "Switch to Chinese" })).toBeInTheDocument();
    expect(within(header).getByRole("button", { name: "About" })).toBeInTheDocument();
    expect(within(header).getByRole("button", { name: "Export Logs" })).toBeInTheDocument();
    expect(within(header).getByRole("button", { name: "How to Copy Links" })).toBeInTheDocument();
    expect(within(header).getByText("Listening to WeChat")).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "Capture Tasks" })).toBeInTheDocument();

    await user.click(within(header).getByRole("button", { name: "Switch to Chinese" }));
    expect(within(header).getByRole("button", { name: "切换为英文" })).toBeInTheDocument();
  });

  it("英文界面会翻译底层返回的公开视频标签", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const template = structuredClone((await backend.listTasks()).find(({ task }) => task.kind === "video")!);
    const video = {
      ...template,
      task: { ...template.task, id: 88, status: "ready" as const },
      video: {
        ...template.video!,
        candidates: [{
          url: "https://finder.video.qq.com/public-demo.mp4",
          kind: "direct_file" as const,
          label: "公开视频文件",
          downloadable: true,
        }],
      },
    };
    backend.listTasks = async () => [video];
    backend.getTaskDetail = async () => video;

    render(<App backend={backend} />);
    const [header] = await screen.findAllByRole("banner");
    await user.click(within(header).getByRole("button", { name: "切换为英文" }));

    expect(await screen.findByText("Public Video File")).toBeInTheDocument();
    expect(screen.queryByText("公开视频文件")).not.toBeInTheDocument();
  });

  it("可在讯栖关于页导出隐私友好的诊断日志并定位文件", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const chooseDestination = vi.spyOn(backend, "chooseDiagnosticDestination");
    const exportDiagnostics = vi.spyOn(backend, "exportDiagnostics");
    const revealOutput = vi.spyOn(backend, "revealOutput");
    render(<App backend={backend} />);

    const [header] = await screen.findAllByRole("banner");
    await user.click(within(header).getByRole("button", { name: "关于" }));
    const dialog = screen.getByRole("dialog", { name: "讯栖 XunQi" });
    await user.click(within(dialog).getByRole("button", { name: "导出诊断日志" }));

    await waitFor(() => expect(chooseDestination).toHaveBeenCalledTimes(1));
    expect(exportDiagnostics).toHaveBeenCalledWith("/tmp/XunQi-Diagnostics-preview.txt");
    expect(revealOutput).toHaveBeenCalledWith("/tmp/XunQi-Diagnostics-preview.txt");
    expect(await screen.findByText(/诊断日志已导出并在文件夹中显示/)).toBeInTheDocument();
  });

  it("在右侧一次处理勾选的公众号并导出 PDF，不把视频号加入批量下载", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const fixtures = await backend.listTasks();
    const readyArticle = structuredClone(fixtures.find(({ task }) => task.id === 1)!);
    const articleTemplate = structuredClone(fixtures.find(({ task }) => task.id === 8)!);
    const video = structuredClone(fixtures.find(({ task }) => task.id === 6)!);
    const unreadArticle = {
      ...articleTemplate,
      task: {
        ...articleTemplate.task,
        id: 105,
        sourceName: "待整理公众号",
        title: "尚未读取的公众号文章",
        status: "failed" as const,
        statusDetail: "等待重新读取",
      },
      article: null,
    };
    const processedArticle = {
      ...unreadArticle,
      task: {
        ...unreadArticle.task,
        status: "ready" as const,
        statusDetail: "内容读取完成，可导出",
      },
      article: {
        ...articleTemplate.article!,
        title: unreadArticle.task.title,
        author: unreadArticle.task.sourceName,
      },
    };
    const records = [readyArticle, unreadArticle, video];
    backend.listTasks = async () => structuredClone(records);
    backend.getTaskDetail = async (taskId) => structuredClone(
      taskId === unreadArticle.task.id
        ? unreadArticle
        : records.find(({ task }) => task.id === taskId)!,
    );
    const processTask = vi.fn(async () => structuredClone(processedArticle));
    const exportArticle = vi.spyOn(backend, "exportArticle");
    const downloadVideo = vi.spyOn(backend, "downloadVideo");
    const chooseOutputDirectory = vi.spyOn(backend, "chooseOutputDirectory");
    backend.processTask = processTask;

    render(<App backend={backend} />);

    const sidebar = await screen.findByRole("complementary", { name: "捕获任务" });
    for (const title of [readyArticle.task.title, unreadArticle.task.title, video.task.title]) {
      await user.click(screen.getByRole("checkbox", { name: `选择${title}` }));
    }

    const main = screen.getByRole("main");
    const batchButton = within(main).getByRole("button", { name: "批量处理并导出 2 篇 PDF" });
    expect(within(sidebar).queryByRole("button", { name: /批量处理/ })).not.toBeInTheDocument();
    await user.click(batchButton);

    await waitFor(() => expect(chooseOutputDirectory).toHaveBeenCalledTimes(1));
    await waitFor(() => expect(exportArticle).toHaveBeenCalledTimes(2));
    expect(processTask).toHaveBeenCalledWith(unreadArticle.task.id);
    expect(exportArticle).toHaveBeenCalledWith(readyArticle.task.id, "/tmp/讯栖预览输出", "pdf");
    expect(exportArticle).toHaveBeenCalledWith(unreadArticle.task.id, "/tmp/讯栖预览输出", "pdf");
    expect(downloadVideo).not.toHaveBeenCalled();
  });

  it("仅勾选视频号时不显示公众号批量入口，也不会自动下载", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const downloadVideo = vi.spyOn(backend, "downloadVideo");
    const chooseOutputDirectory = vi.spyOn(backend, "chooseOutputDirectory");
    render(<App backend={backend} />);

    await screen.findByText("捕获任务");
    await user.click(screen.getByRole("checkbox", { name: /选择测试视频：公开直链识别演示/ }));
    expect(screen.getByText("已选 1 项")).toBeInTheDocument();
    expect(screen.getByText("公众号 0 · 视频号 1")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /批量处理并导出/ })).not.toBeInTheDocument();
    expect(chooseOutputDirectory).not.toHaveBeenCalled();
    expect(downloadVideo).not.toHaveBeenCalled();
  });

  it("Windows 微信位于前台时先建立剪贴板基线，随后复制的新链接会被捕获", async () => {
    const backend = createPreviewBackend();
    const existing = (await backend.listTasks())[0];
    const submitLinks = vi.fn(async () => ({ tasks: [existing.task], duplicateCount: 1 }));
    backend.submitLinks = submitLinks;
    backend.detectWechatForeground = async () => ({
      isWechatFrontmost: true,
      applicationName: "WeChat.exe",
      bundleIdentifier: null,
      limitation: "",
    });
    let clipboardReadCount = 0;
    backend.readClipboardText = async () => {
      clipboardReadCount += 1;
      return clipboardReadCount === 1
        ? "https://mp.weixin.qq.com/s/old-share"
        : "https://mp.weixin.qq.com/s/new-share";
    };

    render(<App backend={backend} />);

    await waitFor(() => expect(submitLinks).toHaveBeenCalledWith("https://mp.weixin.qq.com/s/new-share"));
  });

  it("未读取文章处理失败时会报告并保留该选择", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    render(<App backend={backend} />);

    await screen.findByText("捕获任务");
    await user.click(screen.getByRole("checkbox", { name: /正在读取的公开内容/ }));
    await user.click(screen.getByRole("button", { name: "批量处理并导出 1 篇 PDF" }));

    expect(await screen.findByText(/成功 0 篇，失败 1 篇/)).toBeInTheDocument();
    expect(screen.getByText("已选 1 项")).toBeInTheDocument();
  });

  it("批量 PDF 已保存但状态未同步时会显示具体保存路径", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    backend.exportArticle = vi.fn(async (taskId: number) => ({
      taskId,
      action: "article_pdf" as const,
      destination: "/tmp/xunqi-test-output/公众号文章.pdf",
      bytesWritten: 1024,
      warning: "文件已经保存，但任务状态未能同步。保存位置：/tmp/xunqi-test-output/公众号文章.pdf",
    }));
    render(<App backend={backend} />);

    await screen.findByText("捕获任务");
    await user.click(screen.getByRole("checkbox", { name: /本地内容整理工作流/ }));
    await user.click(screen.getByRole("button", { name: "批量处理并导出 1 篇 PDF" }));

    expect(await screen.findByText((_, element) =>
      element?.classList.contains("toast") === true
      && element.textContent?.includes("状态未能同步") === true
      && element.textContent.includes("/tmp/xunqi-test-output/公众号文章.pdf")
    )).toBeInTheDocument();
  });

  it("同一批次的 PDF 提示和失败会一起报告", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    backend.exportArticle = vi.fn(async (taskId: number) => {
      if (taskId === 2) throw new Error("PDF 渲染失败");
      return {
        taskId,
        action: "article_pdf" as const,
        destination: `/tmp/xunqi-test-output/公众号文章-${taskId}.pdf`,
        bytesWritten: 1024,
        warning: taskId === 1
          ? "保存位置：/tmp/xunqi-test-output/公众号文章-1.pdf"
          : null,
      };
    });
    render(<App backend={backend} />);

    await screen.findByText("捕获任务");
    for (const title of [/本地内容整理工作流/, /三步完成文章导出/, /批量导出示例/]) {
      await user.click(screen.getByRole("checkbox", { name: title }));
    }
    await user.click(screen.getByRole("button", { name: "批量处理并导出 3 篇 PDF" }));

    expect(await screen.findByText((_, element) =>
      element?.classList.contains("toast") === true
      && element.textContent?.includes("成功 2 篇，失败 1 篇") === true
      && element.textContent.includes("另有 1 项提示")
      && element.textContent.includes("/tmp/xunqi-test-output/公众号文章-1.pdf")
    )).toBeInTheDocument();
    expect(screen.getByText("已选 1 项")).toBeInTheDocument();
  });

  it("打开时没有旧任务会显示微信捕获空态", async () => {
    const backend = createPreviewBackend();
    backend.listTasks = async () => [];
    render(<App backend={backend} />);

    expect(await screen.findByText("还没有捕获任务")).toBeInTheDocument();
    expect(screen.getByText("等待微信分享链接")).toBeInTheDocument();
    expect(screen.getByText("自动识别公众号与视频号，无需预先选择来源")).toBeInTheDocument();
  });

  it("完成输出后可以在 Finder 中定位保存结果", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const completed = (await backend.listTasks()).find(({ task }) => task.status === "completed");
    expect(completed).toBeDefined();
    backend.listTasks = async () => [completed!];
    backend.getTaskDetail = async () => completed!;
    const revealOutput = vi.spyOn(backend, "revealOutput");

    render(<App backend={backend} />);

    await user.click(await screen.findByRole("button", { name: "在 Finder 中显示" }));
    expect(revealOutput).toHaveBeenCalledWith(completed!.task.completedPath);
  });

  it("公众号只提供 PDF 和 Markdown，不显示开发者归档包", async () => {
    render(<App backend={createPreviewBackend()} />);

    expect(await screen.findByRole("option", { name: "PDF（保留原文结构）" })).toBeInTheDocument();
    expect(screen.getByRole("option", { name: "Markdown + 本地图片" })).toBeInTheDocument();
    expect(screen.queryByRole("option", { name: "文章 + 图片 + 元数据" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "生成归档包" })).not.toBeInTheDocument();
  });

  it("在应用内说明公众号与视频号如何复制微信链接", async () => {
    const user = userEvent.setup();
    render(<App backend={createPreviewBackend()} />);

    await user.click(await screen.findByRole("button", { name: "如何复制链接" }));

    const dialog = screen.getByRole("dialog", { name: "如何复制微信链接" });
    expect(within(dialog).getByText("公众号文章")).toBeInTheDocument();
    expect(within(dialog).getByText(/右上角的三个点/)).toBeInTheDocument();
    expect(within(dialog).getAllByText("复制链接")).toHaveLength(2);
    expect(within(dialog).getByText("视频号")).toBeInTheDocument();
    expect(within(dialog).getByText(/分享按钮/)).toBeInTheDocument();
  });

  it("零直链视频必须明确授权后才启动嗅探助手，且不提供录制保存", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const zeroLinkVideo = {
      task: {
        ...(await backend.listTasks()).find(({ task }) => task.kind === "video")!.task,
        id: 88,
        title: "需要授权捕获的视频号",
        status: "needs_attention" as const,
        statusDetail: "微信公开页面没有提供视频地址",
      },
      article: null,
      video: {
        pageTitle: "需要授权捕获的视频号",
        pageUrl: "https://weixin.qq.com/sph/test-authorized-sniff",
        sourceName: "测试视频号",
        description: "",
        publishedAt: null,
        coverImageUrl: null,
        candidates: [],
        limitation: "微信公开页面没有提供视频地址",
      },
    };
    backend.listTasks = async () => [zeroLinkVideo];
    backend.getTaskDetail = async () => zeroLinkVideo;
    const prepareVideoSniff = vi.fn(async () => ({
      planId: "plan-88",
      taskId: 88,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: true,
      changes: ["temporary_proxy", "temporary_certificate"] as const,
      reusesAuthorization: false,
      helperSource: "ltaoo/wx_channels_download v260706",
      conflict: null,
    }));
    const startVideoSniff = vi.fn(async () => ({
      sessionId: "session-88",
      taskId: 88,
      phase: "saving" as const,
      message: "已重新加载视频号子窗口；无需刷新，也不用寻找页面下载按钮。",
      helperPageUrl: "http://127.0.0.1:2022/download",
      output: null,
      errorCode: null,
      destinationDirectory: "/tmp/xunqi-test-downloads",
      authorizationReusable: true,
      progress: {
        downloadedBytes: 37 * 1024 * 1024,
        totalBytes: 100 * 1024 * 1024,
        bytesPerSecond: 1.5 * 1024 * 1024,
        percent: 37,
      },
    }));
    const openExternal = vi.fn(async (url: string) => {
      if (url.startsWith("http://127.0.0.1:")) {
        throw new Error("本地助手地址被拦截");
      }
    });
    Object.assign(backend, {
      prepareVideoSniff,
      startVideoSniff,
      getVideoSniffSession: startVideoSniff,
      stopVideoSniff: startVideoSniff,
      openExternal,
      recoverVideoSniffing: async () => ({ recovered: true, message: "系统网络设置正常" }),
    });

    render(<App backend={backend} />);

    expect(await screen.findByRole("button", { name: "授权嗅探下载" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "录制保存" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "授权嗅探下载" }));
    const authorizationDialog = await screen.findByRole("dialog", { name: "启用授权嗅探助手" });
    expect(authorizationDialog).toBeInTheDocument();
    expect(within(authorizationDialog).getByText(/系统 HTTP\/HTTPS 流量/)).toBeInTheDocument();
    expect(within(authorizationDialog).getByText(/不是零风险功能/)).toBeInTheDocument();
    expect(within(authorizationDialog).getByText(/VPN 与系统代理必须在启用前关闭/)).toBeInTheDocument();
    expect(within(authorizationDialog).getByText(/首次授权.*重新加载/)).toBeInTheDocument();
    expect(within(authorizationDialog).getByText(/按已复制的分享链接自动下载/)).toBeInTheDocument();
    expect(within(authorizationDialog).getByText(/不要转到浏览器扫码/)).toBeInTheDocument();
    expect(within(authorizationDialog).queryByText(/Command\+R/)).not.toBeInTheDocument();
    expect(within(authorizationDialog).queryByText(/Command \+ Q/)).not.toBeInTheDocument();
    expect(within(authorizationDialog).queryByText(/完全退出微信/)).not.toBeInTheDocument();
    expect(within(authorizationDialog).queryByText(/只处理微信视频号域名/)).not.toBeInTheDocument();
    expect(startVideoSniff).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "取消" }));
    expect(startVideoSniff).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "授权嗅探下载" }));
    await user.click(await screen.findByRole("button", { name: "同意并启用" }));
    await waitFor(() => expect(startVideoSniff).toHaveBeenCalledTimes(1));
    expect(openExternal).not.toHaveBeenCalledWith(
      "https://weixin.qq.com/sph/test-authorized-sniff",
    );
    expect((await screen.findAllByText(/无需刷新，也不用寻找页面下载按钮/)).length).toBeGreaterThan(0);
    expect(screen.getByText("37%")).toBeInTheDocument();
    expect(screen.getByText(/1\.5 MB\/s/)).toBeInTheDocument();
    expect(screen.getAllByText(/保存到 \/tmp\/xunqi-test-downloads/).length).toBeGreaterThan(0);

    expect(screen.queryByRole("button", { name: "打开下载助手" })).not.toBeInTheDocument();

    const [header] = await screen.findAllByRole("banner");
    await user.click(within(header).getByRole("button", { name: "切换为英文" }));
    expect(screen.getByText("Original Quality")).toBeInTheDocument();
    expect(screen.queryByText("原始画质")).not.toBeInTheDocument();
  });

  it("Windows 英文界面会准确说明授权嗅探的平台边界", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const template = structuredClone((await backend.listTasks()).find(({ task }) => task.kind === "video")!);
    const video = {
      ...template,
      task: { ...template.task, id: 89, status: "needs_attention" as const, completedPath: null },
      video: { ...template.video!, candidates: [] },
    };
    backend.listTasks = async () => [video];
    backend.getTaskDetail = async () => video;
    backend.prepareVideoSniff = async () => ({
      planId: "",
      taskId: 89,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: false,
      changes: [],
      reusesAuthorization: false,
      helperSource: "",
      conflict: {
        code: "windows_sniffer_unavailable",
        message: "Windows 测试版暂未提供授权嗅探下载；这不是 WebView2 缺失。",
      },
    });

    render(<App backend={backend} platform="windows" />);
    const [header] = await screen.findAllByRole("banner");
    await user.click(within(header).getByRole("button", { name: "切换为英文" }));

    expect(await screen.findByText("Authorized Detection Is Not Available on Windows Yet")).toBeInTheDocument();
    expect(screen.getByText(/not a missing WebView2 component/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Authorize Detection" })).not.toBeInTheDocument();
  });

  it("首次授权后的下一条视频直接复用授权和保存目录", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const template = (await backend.listTasks()).find(({ task }) => task.kind === "video")!;
    const videos = [101, 102].map((id) => ({
      task: {
        ...template.task,
        id,
        title: `待下载视频 ${id}`,
        sourceName: `测试视频号 ${id}`,
        status: "needs_attention" as const,
        completedPath: null,
      },
      article: null,
      video: {
        ...template.video!,
        pageTitle: `待下载视频 ${id}`,
        sourceName: `测试视频号 ${id}`,
        candidates: [],
      },
    }));
    backend.listTasks = async () => videos;
    backend.getTaskDetail = async (taskId) => videos.find(({ task }) => task.id === taskId)!;
    const prepareVideoSniff = vi.fn(async (taskId: number) => ({
      planId: `plan-${taskId}`,
      taskId,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: true,
      changes: taskId === 101 ? ["temporary_proxy", "temporary_certificate"] as const : [],
      reusesAuthorization: taskId === 102,
      helperSource: "ltaoo/wx_channels_download v260706",
      conflict: null,
    }));
    const startVideoSniff = vi.fn(async (taskId: number) => ({
      sessionId: `session-${taskId}`,
      taskId,
      phase: "completed" as const,
      message: "已保存，连续下载仍已启用。",
      helperPageUrl: "http://127.0.0.1:2022/download",
      destinationDirectory: "/tmp/xunqi-test-downloads",
      authorizationReusable: true,
      progress: null,
      output: null,
      errorCode: null,
    }));
    const chooseOutputDirectory = vi.fn(async () => "/tmp/xunqi-test-downloads");
    Object.assign(backend, { prepareVideoSniff, startVideoSniff, chooseOutputDirectory });

    render(<App backend={backend} />);

    await user.click(await screen.findByRole("button", { name: "授权嗅探下载" }));
    await user.click(await screen.findByRole("button", { name: "同意并启用" }));
    await waitFor(() => expect(startVideoSniff).toHaveBeenCalledTimes(1));
    await user.click(screen.getByRole("button", { name: /待下载视频 102/ }));
    await user.click(await screen.findByRole("button", { name: "嗅探下载" }));

    await waitFor(() => expect(startVideoSniff).toHaveBeenCalledTimes(2));
    expect(chooseOutputDirectory).toHaveBeenCalledTimes(1);
    expect(startVideoSniff).toHaveBeenLastCalledWith(
      102,
      "plan-102",
      "/tmp/xunqi-test-downloads",
      "original",
    );
    expect(screen.queryByRole("dialog", { name: "启用授权嗅探助手" })).not.toBeInTheDocument();
  });

  it("勾选多个视频后按单任务校验队列自动接续，并把节省空间模式传到底层", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const template = (await backend.listTasks()).find(({ task }) => task.kind === "video")!;
    const videos = [201, 202].map((id) => ({
      task: {
        ...template.task,
        id,
        title: `队列视频 ${id}`,
        sourceName: `队列来源 ${id}`,
        status: "needs_attention" as const,
        completedPath: null,
      },
      article: null,
      video: {
        ...template.video!,
        pageTitle: `队列视频 ${id}`,
        sourceName: `队列来源 ${id}`,
        candidates: [],
      },
    }));
    backend.listTasks = async () => videos;
    backend.getTaskDetail = async (taskId) => videos.find(({ task }) => task.id === taskId)!;
    const prepareVideoSniff = vi.fn(async (taskId: number) => ({
      planId: `plan-${taskId}`,
      taskId,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: true,
      changes: taskId === 201 ? ["temporary_proxy", "temporary_certificate"] as const : [],
      reusesAuthorization: taskId === 202,
      helperSource: "ltaoo/wx_channels_download v260706",
      conflict: null,
    }));
    const startVideoSniff = vi.fn(async (taskId: number) => ({
      sessionId: `session-${taskId}`,
      taskId,
      phase: "completed" as const,
      message: `队列视频 ${taskId} 已保存`,
      helperPageUrl: "http://127.0.0.1:2022/download",
      destinationDirectory: "/tmp/xunqi-test-downloads",
      authorizationReusable: true,
      progress: {
        downloadedBytes: 100 * 1024 * 1024,
        totalBytes: 100 * 1024 * 1024,
        bytesPerSecond: 0,
        percent: 100,
      },
      output: {
        destination: `/tmp/xunqi-test-downloads/${taskId}.mp4`,
        bytesWritten: 100 * 1024 * 1024,
        qualityLabel: "原始画质",
        width: 1920,
        height: 1080,
      },
      errorCode: null,
    }));
    const chooseOutputDirectory = vi.fn(async () => "/tmp/xunqi-test-downloads");
    Object.assign(backend, { prepareVideoSniff, startVideoSniff, chooseOutputDirectory });

    render(<App backend={backend} />);

    await user.selectOptions(await screen.findByLabelText("视频下载画质"), "space_saver");
    await user.click(await screen.findByRole("checkbox", { name: "选择队列视频 201" }));
    await user.click(screen.getByRole("checkbox", { name: "选择队列视频 202" }));
    await user.click(screen.getByRole("button", { name: "开始连续下载 2 条" }));
    await user.click(await screen.findByRole("button", { name: "同意并启用" }));

    await waitFor(() => expect(startVideoSniff).toHaveBeenCalledTimes(2));
    expect(startVideoSniff).toHaveBeenNthCalledWith(1, 201, "plan-201", "/tmp/xunqi-test-downloads", "space_saver");
    expect(startVideoSniff).toHaveBeenNthCalledWith(2, 202, "plan-202", "/tmp/xunqi-test-downloads", "space_saver");
    expect(chooseOutputDirectory).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/队列完成 2\/2/)).toBeInTheDocument();
  });

  it("批量队列会跳过已经保存到本地的视频，不会再次等待它们播放", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const template = (await backend.listTasks()).find(({ task }) => task.kind === "video")!;
    const saved = {
      ...structuredClone(template),
      task: {
        ...template.task,
        id: 301,
        title: "已保存视频",
        status: "needs_attention" as const,
        completedPath: "/tmp/已保存视频.mp4",
      },
      video: { ...template.video!, candidates: [] },
    };
    const pending = {
      ...structuredClone(template),
      task: {
        ...template.task,
        id: 302,
        title: "待下载视频",
        status: "needs_attention" as const,
        completedPath: null,
      },
      video: { ...template.video!, candidates: [] },
    };
    backend.listTasks = async () => [saved, pending];
    backend.getTaskDetail = async (taskId) => taskId === saved.task.id ? saved : pending;
    const prepareVideoSniff = vi.fn(async (taskId: number) => ({
      planId: `plan-${taskId}`,
      taskId,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: true,
      changes: ["temporary_proxy", "temporary_certificate"] as const,
      reusesAuthorization: false,
      helperSource: "ltaoo/wx_channels_download v260706",
      conflict: null,
    }));
    Object.assign(backend, { prepareVideoSniff });

    render(<App backend={backend} />);

    await user.click(await screen.findByRole("button", { name: "全选全部 2 项" }));
    expect(screen.getByRole("button", { name: "开始连续下载 1 条" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "开始连续下载 1 条" }));
    expect(prepareVideoSniff).toHaveBeenCalledWith(302);
    expect(prepareVideoSniff).not.toHaveBeenCalledWith(301);
  });

  it("队列任务失败并已恢复网络时会结束处理状态，不再永久转圈", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const template = (await backend.listTasks()).find(({ task }) => task.kind === "video")!;
    const videos = [401, 402].map((id) => ({
      ...structuredClone(template),
      task: {
        ...template.task,
        id,
        title: `失败队列视频 ${id}`,
        status: "needs_attention" as const,
        completedPath: null,
      },
      video: { ...template.video!, candidates: [] },
    }));
    backend.listTasks = async () => videos;
    backend.getTaskDetail = async (taskId) => videos.find(({ task }) => task.id === taskId)!;
    backend.prepareVideoSniff = vi.fn(async (taskId: number) => ({
      planId: `plan-${taskId}`,
      taskId,
      expiresAt: "2099-01-01T00:00:00Z",
      canStart: true,
      changes: ["temporary_proxy", "temporary_certificate"] as const,
      reusesAuthorization: false,
      helperSource: "ltaoo/wx_channels_download v260706",
      conflict: null,
    }));
    backend.startVideoSniff = vi.fn(async (taskId: number) => ({
      sessionId: `session-${taskId}`,
      taskId,
      phase: "failed_restored" as const,
      message: "等待视频号连接超时，网络设置已恢复。",
      helperPageUrl: null,
      destinationDirectory: "/tmp/xunqi-test-downloads",
      authorizationReusable: false,
      progress: null,
      output: null,
      errorCode: "capture_timeout",
    }));

    render(<App backend={backend} />);

    await user.click(await screen.findByRole("button", { name: "全选全部 2 项" }));
    await user.click(screen.getByRole("button", { name: "开始连续下载 2 条" }));
    await user.click(await screen.findByRole("button", { name: "同意并启用" }));

    await waitFor(() => expect(screen.queryByRole("button", { name: "队列处理中" })).not.toBeInTheDocument());
    expect(screen.getByRole("button", { name: "开始连续下载 2 条" })).toBeInTheDocument();
    expect(screen.getAllByText(/等待视频号连接超时/).length).toBeGreaterThan(0);
  });

  it("启动恢复失败时保留阻断式恢复入口，恢复成功后才移除", async () => {
    const user = userEvent.setup();
    const backend = createPreviewBackend();
    const recoverVideoSniffing = vi
      .fn()
      .mockRejectedValueOnce(new Error("代理设置已被其他程序修改"))
      .mockResolvedValueOnce({ recovered: true, message: "网络设置已恢复" });
    backend.recoverVideoSniffing = recoverVideoSniffing;

    render(<App backend={backend} />);

    const alert = await screen.findByRole("alert");
    expect(within(alert).getByText("授权助手的网络设置还没有恢复")).toBeInTheDocument();
    await user.click(within(alert).getByRole("button", { name: "立即恢复网络设置" }));
    await waitFor(() => expect(screen.queryByRole("alert")).not.toBeInTheDocument());
    expect(recoverVideoSniffing).toHaveBeenCalledTimes(2);
  });
});
