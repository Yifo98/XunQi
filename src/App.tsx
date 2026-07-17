import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArticleIcon,
  CaretRightIcon,
  CheckCircleIcon,
  CopySimpleIcon,
  FileArrowDownIcon,
  InfoIcon,
  LinkSimpleIcon,
  QuestionIcon,
  ShareNetworkIcon,
  SpinnerGapIcon,
  VideoCameraIcon,
  WarningCircleIcon,
  XIcon,
} from "@phosphor-icons/react";
import logoUrl from "../assets/brand/xunqi-ui.png";
import "./App.css";
import { BrandAboutDialog } from "./components/BrandAboutDialog";
import { TaskDetail } from "./components/TaskDetail";
import { TaskSidebar, type KindFilter } from "./components/TaskSidebar";
import { LanguageProvider, useI18n } from "./i18n";
import {
  isTauriRuntime,
  tauriBackend,
  type ArticleExportMode,
  type Backend,
  type CaptureTaskDetail,
  type DetectedVideo,
  type SniffAuthorizationPlan,
  type SniffQualityMode,
  type SniffSessionSnapshot,
} from "./lib/backend";
import { createPreviewBackend } from "./lib/previewBackend";

type AppProps = {
  backend?: Backend;
  platform?: "macos" | "windows" | "other";
};

type SniffQueueState = {
  taskIds: number[];
  currentIndex: number;
  destinationDirectory: string;
  status: "awaiting_authorization" | "running" | "completed" | "stopped";
  failedCount: number;
};

const defaultBackend = isTauriRuntime() ? tauriBackend : createPreviewBackend();

function AppContent({ backend = defaultBackend, platform = detectPlatform() }: AppProps) {
  const { language, setLanguage, text } = useI18n();
  const authorizedSniffSupported = platform === "macos" || platform === "windows";
  const authorizedSniffExperimental = platform === "windows";
  const [tasks, setTasks] = useState<CaptureTaskDetail[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [kindFilter, setKindFilter] = useState<KindFilter>("all");
  const [activeTaskId, setActiveTaskId] = useState<number | null>(null);
  const [selectedTaskIds, setSelectedTaskIds] = useState<Set<number>>(new Set());
  const [collapsedSources, setCollapsedSources] = useState<Set<string>>(new Set());
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [busyTaskIds, setBusyTaskIds] = useState<Set<number>>(new Set());
  const [batchProgress, setBatchProgress] = useState<{ current: number; total: number } | null>(null);
  const [toast, setToast] = useState<{ kind: "success" | "warning" | "error"; message: string } | null>(null);
  const [listeningActivity, setListeningActivity] = useState("自动识别公众号与视频号，无需预先选择来源");
  const [sniffPlan, setSniffPlan] = useState<SniffAuthorizationPlan | null>(null);
  const [sniffSession, setSniffSession] = useState<SniffSessionSnapshot | null>(null);
  const [sniffQueue, setSniffQueue] = useState<SniffQueueState | null>(null);
  const [videoQualityMode, setVideoQualityMode] = useState<SniffQualityMode>("original");
  const [sniffRecoveryNeeded, setSniffRecoveryNeeded] = useState(false);
  const [showLinkGuide, setShowLinkGuide] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [diagnosticsBusy, setDiagnosticsBusy] = useState(false);
  const clipboardFingerprintRef = useRef<string | null>(null);
  const captureBusyRef = useRef(false);
  const wechatWasFrontmostRef = useRef(false);
  const advancedSniffSessionsRef = useRef<Set<string>>(new Set());

  const reload = useCallback(
    async (preferredTaskId?: number) => {
      const next = await backend.listTasks();
      setTasks((current) => mergeTaskSummaries(current, next));
      setActiveTaskId((current) => {
        const preferred = preferredTaskId ?? current;
        if (preferred && next.some(({ task }) => task.id === preferred)) return preferred;
        return next[0]?.task.id ?? null;
      });
      return next;
    },
    [backend],
  );

  useEffect(() => {
    void backend.recoverVideoSniffing()
      .then((result) => setSniffRecoveryNeeded(!result.recovered))
      .catch((reason) => {
        setSniffRecoveryNeeded(true);
        setToast({
          kind: "error",
          message: `检测到上次授权会话未恢复：${readableError(reason)}`,
        });
      });
  }, [backend]);

  useEffect(() => {
    if (!sniffSession || !isActiveSniffPhase(sniffSession.phase)) return;
    let disposed = false;
    const poll = window.setInterval(() => {
      void backend.getVideoSniffSession(sniffSession.sessionId).then((snapshot) => {
        if (disposed) return;
        setSniffSession(snapshot);
        if (snapshot.phase === "completed") {
          void reload(snapshot.taskId);
          setListeningActivity("视频已保存；连续授权仍可用于下一条");
        }
      }).catch((reason) => {
        if (!disposed) setToast({ kind: "error", message: readableError(reason) });
      });
    }, 1000);
    return () => {
      disposed = true;
      window.clearInterval(poll);
    };
  }, [backend, reload, sniffSession]);

  useEffect(() => {
    if (!sniffQueue || sniffQueue.status !== "running" || !sniffSession) return;
    if (sniffQueue.taskIds[sniffQueue.currentIndex] !== sniffSession.taskId) return;
    if (["failed_restored", "cancelled_restored", "restoration_required"].includes(sniffSession.phase)) {
      const stopTimer = window.setTimeout(() => {
        setSniffQueue((current) => current ? { ...current, status: "stopped" } : current);
        setSniffRecoveryNeeded(sniffSession.phase === "restoration_required");
        setListeningActivity(sniffSession.message);
        setToast({
          kind: sniffSession.phase === "cancelled_restored" ? "warning" : "error",
          message: sniffSession.message,
        });
        void reload(sniffSession.taskId);
      }, 0);
      return () => window.clearTimeout(stopTimer);
    }
    if (!["completed", "failed_reusable"].includes(sniffSession.phase)) return;
    if (advancedSniffSessionsRef.current.has(sniffSession.sessionId)) return;
    const advanceTimer = window.setTimeout(() => {
      if (advancedSniffSessionsRef.current.has(sniffSession.sessionId)) return;
      advancedSniffSessionsRef.current.add(sniffSession.sessionId);

      const failedCount = sniffQueue.failedCount + (sniffSession.phase === "failed_reusable" ? 1 : 0);
      const nextIndex = sniffQueue.currentIndex + 1;
      void reload(sniffSession.taskId);
      if (nextIndex >= sniffQueue.taskIds.length) {
        setSniffQueue({ ...sniffQueue, currentIndex: sniffQueue.taskIds.length - 1, status: "completed", failedCount });
        setListeningActivity(failedCount > 0
          ? `连续下载结束：成功 ${sniffQueue.taskIds.length - failedCount} 条，失败 ${failedCount} 条`
          : `连续下载完成：${sniffQueue.taskIds.length} 条视频已保存`);
        return;
      }

      const nextTaskId = sniffQueue.taskIds[nextIndex];
      const nextQueue = { ...sniffQueue, currentIndex: nextIndex, failedCount };
      setSniffQueue(nextQueue);
      setActiveTaskId(nextTaskId);
      void backend.prepareVideoSniff(nextTaskId).then(async (plan) => {
        if (!plan.canStart) {
          setSniffQueue({ ...nextQueue, status: "stopped" });
          setToast({
            kind: "error",
            message: sniffConflictMessage(
              plan.conflict,
              language,
              "队列中的下一条无法启动",
              "The next queue item cannot start",
            ),
          });
          return;
        }
        if (!plan.reusesAuthorization) {
          setSniffQueue({ ...nextQueue, status: "awaiting_authorization" });
          setSniffPlan(plan);
          return;
        }
        const session = await backend.startVideoSniff(
          nextTaskId,
          plan.planId,
          sniffQueue.destinationDirectory,
          videoQualityMode,
        );
        setSniffSession(session);
        setSniffQueue({ ...nextQueue, status: "running" });
      }).catch((reason) => {
        setSniffQueue({ ...nextQueue, status: "stopped" });
        setToast({ kind: "error", message: readableError(reason) });
      });
    }, 0);
    return () => window.clearTimeout(advanceTimer);
  }, [backend, language, reload, sniffQueue, sniffSession, videoQualityMode]);

  const replaceTask = useCallback((next: CaptureTaskDetail) => {
    setTasks((current) => {
      const exists = current.some(({ task }) => task.id === next.task.id);
      return exists
        ? current.map((item) => (item.task.id === next.task.id ? next : item))
        : [next, ...current];
    });
  }, []);

  const processTask = useCallback(
    async (taskId: number, quiet = false) => {
      setBusyTaskIds((current) => new Set(current).add(taskId));
      setTasks((current) =>
        current.map((item) =>
          item.task.id === taskId
            ? {
                ...item,
                task: {
                  ...item.task,
                  status: "processing",
                  statusDetail: item.task.kind === "article" ? "正在读取公开文章正文…" : "正在检查页面中的公开视频…",
                },
              }
            : item,
        ),
      );
      try {
        const next = await backend.processTask(taskId);
        replaceTask(next);
        if (!quiet) setToast({ kind: "success", message: next.task.statusDetail });
        return next;
      } catch (reason) {
        await reload(taskId);
        if (!quiet) setToast({ kind: "error", message: readableError(reason) });
        return null;
      } finally {
        setBusyTaskIds((current) => withoutId(current, taskId));
      }
    },
    [backend, reload, replaceTask],
  );

  const handleQueryChange = useCallback((value: string) => {
    if (!isWechatShareLink(value)) {
      setQuery(value);
      return;
    }

    setQuery("");
    setListeningActivity("发现微信分享链接，正在自动分类…");
    void backend.submitLinks(value.trim()).then(async (submitted) => {
      const newest = submitted.tasks[0];
      if (!newest) return;
      await reload(newest.id);
      setActiveTaskId(newest.id);
      if (submitted.duplicateCount > 0 && submitted.tasks.every((task) => task.status !== "queued")) {
        setListeningActivity("这条分享链接已经在任务队列中");
        setToast({ kind: "warning", message: "这条微信分享链接已经收取过" });
        return;
      }
      for (const task of submitted.tasks.filter((item) => item.status === "queued")) {
        void processTask(task.id, true).then((processed) => {
          if (processed) setListeningActivity(processed.task.kind === "article"
            ? "文章已读取，可导出原版 PDF"
            : "视频号链接已识别，可选择下载方式");
        });
      }
      setToast({ kind: "success", message: "微信分享链接已进入捕获任务" });
    }).catch((reason) => {
      const message = readableError(reason);
      setListeningActivity(message);
      setToast({ kind: "error", message });
    });
  }, [backend, processTask, reload]);

  useEffect(() => {
    let disposed = false;
    backend
      .listTasks()
      .then((next) => {
        if (disposed) return;
        setTasks(next);
        setActiveTaskId(next[0]?.task.id ?? null);
        const queued = next.filter(({ task }) => task.status === "queued");
        void (async () => {
          for (const { task } of queued) {
            if (disposed) return;
            await processTask(task.id, true);
          }
        })();
      })
      .catch((reason: unknown) => {
        if (!disposed) setLoadError(readableError(reason));
      })
      .finally(() => {
        if (!disposed) setIsLoading(false);
      });
    return () => {
      disposed = true;
    };
  }, [backend, processTask]);

  useEffect(() => {
    if (activeTaskId === null) return;
    let disposed = false;
    backend
      .getTaskDetail(activeTaskId)
      .then((detail) => {
        if (!disposed) replaceTask(detail);
      })
      .catch((reason: unknown) => {
        if (!disposed) setToast({ kind: "error", message: readableError(reason) });
      });
    return () => {
      disposed = true;
    };
  }, [activeTaskId, backend, replaceTask]);

  useEffect(() => {
    let disposed = false;

    async function probeWechatClipboard() {
      if (captureBusyRef.current) return;
      captureBusyRef.current = true;
      try {
        const foreground = await backend.detectWechatForeground();
        if (disposed) return;
        if (!foreground.isWechatFrontmost) {
          wechatWasFrontmostRef.current = false;
          return;
        }
        const clipboard = (await backend.readClipboardText()).trim();
        if (disposed) return;
        const fingerprint = await clipboardFingerprint(clipboard);
        if (!wechatWasFrontmostRef.current || clipboardFingerprintRef.current === null) {
          wechatWasFrontmostRef.current = true;
          clipboardFingerprintRef.current = fingerprint;
          return;
        }
        if (!clipboard) return;
        if (fingerprint === clipboardFingerprintRef.current) return;
        clipboardFingerprintRef.current = fingerprint;

        setListeningActivity("发现新的微信分享链接，正在自动分类…");
        const submitted = await backend.submitLinks(clipboard);
        const newest = submitted.tasks[0];
        if (!newest) return;
        await reload(newest.id);
        setActiveTaskId(newest.id);
        if (submitted.duplicateCount > 0 && submitted.tasks.every((task) => task.status !== "queued")) {
          setListeningActivity("这条分享链接已经在任务队列中");
          return;
        }
        for (const task of submitted.tasks.filter((item) => item.status === "queued")) {
          void processTask(task.id, true).then((processed) => {
            if (processed) setListeningActivity(processed.task.kind === "article"
              ? "文章已读取，可导出原版 PDF"
              : "视频号链接已识别，可选择下载方式");
          });
        }
        setToast({ kind: "success", message: "微信分享链接已进入捕获任务" });
      } catch (reason) {
        const message = readableError(reason);
        if (!message.includes("没有找到支持的微信")) {
          setListeningActivity(message);
        }
      } finally {
        captureBusyRef.current = false;
      }
    }

    void probeWechatClipboard();
    const interval = window.setInterval(() => void probeWechatClipboard(), 250);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [backend, processTask, reload]);

  const filteredTasks = useMemo(() => {
    const normalizedQuery = normalizeSearch(query);
    return tasks.filter(({ task }) => {
      if (kindFilter !== "all" && task.kind !== kindFilter) return false;
      if (!normalizedQuery) return true;
      return normalizeSearch(`${task.sourceName} ${task.title} ${task.author}`).includes(normalizedQuery);
    });
  }, [kindFilter, query, tasks]);

  const groups = useMemo(() => {
    const grouped = new Map<string, CaptureTaskDetail[]>();
    for (const detail of filteredTasks) {
      const sourceName = detail.task.sourceName || (detail.task.kind === "article" ? "微信公众号" : "视频号");
      const group = grouped.get(sourceName) ?? [];
      group.push(detail);
      grouped.set(sourceName, group);
    }
    return Array.from(grouped, ([sourceName, groupedTasks]) => ({
      sourceName,
      tasks: groupedTasks.sort((left, right) => right.task.createdAt.localeCompare(left.task.createdAt)),
    }));
  }, [filteredTasks]);

  const activeDetail = tasks.find(({ task }) => task.id === activeTaskId) ?? null;
  const articleCount = tasks.filter(({ task }) => task.kind === "article").length;
  const videoCount = tasks.filter(({ task }) => task.kind === "video").length;
  const selectedArticleCount = tasks.filter(
    ({ task }) => task.kind === "article" && selectedTaskIds.has(task.id),
  ).length;
  const selectedVideoCount = tasks.filter(
    ({ task }) => task.kind === "video" && selectedTaskIds.has(task.id),
  ).length;
  const selectedSniffVideoCount = authorizedSniffSupported
    ? tasks.filter(
        (detail) => selectedTaskIds.has(detail.task.id) && needsAuthorizedSniffDownload(detail),
      ).length
    : 0;
  const selectedSkippedVideoCount = selectedVideoCount - selectedSniffVideoCount;
  const activeBusy = activeTaskId !== null && busyTaskIds.has(activeTaskId);
  const visibleTaskIds = filteredTasks.map(({ task }) => task.id);
  const allVisibleSelected = visibleTaskIds.length > 0
    && visibleTaskIds.every((taskId) => selectedTaskIds.has(taskId));

  async function handleExport(taskId: number, mode: ArticleExportMode) {
    const directory = await backend.chooseOutputDirectory();
    if (!directory) return;
    setBusyTaskIds((current) => new Set(current).add(taskId));
    try {
      const result = await backend.exportArticle(taskId, directory, mode);
      await reload(taskId);
      setToast({
        kind: result.warning ? "warning" : "success",
        message: result.warning
          ?? (mode === "pdf" ? `原版 PDF 已保存到 ${result.destination}` : `文章已导出到 ${result.destination}`),
      });
    } catch (reason) {
      await reload(taskId);
      setToast({ kind: "error", message: readableError(reason) });
    } finally {
      setBusyTaskIds((current) => withoutId(current, taskId));
    }
  }

  async function handleDownload(taskId: number, candidate: DetectedVideo) {
    const directory = await backend.chooseOutputDirectory();
    if (!directory) return;
    setBusyTaskIds((current) => new Set(current).add(taskId));
    try {
      const result = await backend.downloadVideo(taskId, candidate.url, directory);
      await reload(taskId);
      setToast({
        kind: result.warning ? "warning" : "success",
        message: result.warning ?? `视频已下载到 ${result.destination}`,
      });
    } catch (reason) {
      await reload(taskId);
      setToast({ kind: "error", message: readableError(reason) });
    } finally {
      setBusyTaskIds((current) => withoutId(current, taskId));
    }
  }

  async function handlePrepareSniff(taskId: number) {
    setBusyTaskIds((current) => new Set(current).add(taskId));
    try {
      const plan = await backend.prepareVideoSniff(taskId);
      if (!plan.canStart) {
        if (plan.conflict?.code === "recovery_required") setSniffRecoveryNeeded(true);
        setToast({
          kind: "error",
          message: sniffConflictMessage(
            plan.conflict,
            language,
            "当前不能安全启用授权嗅探助手，可稍后重试或打开微信原文。",
            "Authorized detection cannot be enabled safely right now. Try again later or open the original in WeChat.",
          ),
        });
        return;
      }
      if (plan.reusesAuthorization) {
        await startSniffWithPlan(plan);
      } else {
        setSniffPlan(plan);
      }
    } catch (reason) {
      setToast({ kind: "error", message: readableError(reason) });
    } finally {
      setBusyTaskIds((current) => withoutId(current, taskId));
    }
  }

  async function handleStartSniff() {
    const plan = sniffPlan;
    if (!plan) return;
    const queueDirectory = sniffQueue?.taskIds.includes(plan.taskId)
      ? sniffQueue.destinationDirectory
      : undefined;
    await startSniffWithPlan(plan, queueDirectory);
  }

  async function startSniffWithPlan(plan: SniffAuthorizationPlan, forcedDirectory?: string) {
    const reusableDirectory = forcedDirectory || (plan.reusesAuthorization
      ? sniffSession?.destinationDirectory
      : null);
    const directory = reusableDirectory || await backend.chooseOutputDirectory();
    if (!directory) return;
    setSniffPlan(null);
    setBusyTaskIds((current) => new Set(current).add(plan.taskId));
    try {
      const session = await backend.startVideoSniff(
        plan.taskId,
        plan.planId,
        directory,
        videoQualityMode,
      );
      setSniffSession(session);
      setActiveTaskId(plan.taskId);
      setSniffQueue((current) => current?.taskIds.includes(plan.taskId)
        ? { ...current, status: "running" }
        : current);
      setToast(null);
    } catch (reason) {
      setToast({ kind: "error", message: readableError(reason) });
      setSniffQueue((current) => current?.taskIds.includes(plan.taskId)
        ? { ...current, status: "stopped" }
        : current);
    } finally {
      setBusyTaskIds((current) => withoutId(current, plan.taskId));
    }
  }

  async function handleStartVideoQueue() {
    const taskIds = tasks
      .filter((detail) => selectedTaskIds.has(detail.task.id) && needsAuthorizedSniffDownload(detail))
      .map(({ task }) => task.id);
    if (taskIds.length === 0) {
      setToast({ kind: "error", message: "勾选的视频里没有需要授权嗅探的任务" });
      return;
    }
    const directory = await backend.chooseOutputDirectory();
    if (!directory) return;
    const queue: SniffQueueState = {
      taskIds,
      currentIndex: 0,
      destinationDirectory: directory,
      status: "awaiting_authorization",
      failedCount: 0,
    };
    setSniffQueue(queue);
    setActiveTaskId(taskIds[0]);
    try {
      const plan = await backend.prepareVideoSniff(taskIds[0]);
      if (!plan.canStart) {
        setSniffQueue({ ...queue, status: "stopped" });
        setToast({
          kind: "error",
          message: sniffConflictMessage(
            plan.conflict,
            language,
            "当前不能启动连续下载队列",
            "The continuous download queue cannot start",
          ),
        });
        return;
      }
      if (plan.reusesAuthorization) {
        await startSniffWithPlan(plan, directory);
      } else {
        setSniffPlan(plan);
      }
    } catch (reason) {
      setSniffQueue({ ...queue, status: "stopped" });
      setToast({ kind: "error", message: readableError(reason) });
    }
  }

  async function handleStopSniff(sessionId: string) {
    try {
      const session = await backend.stopVideoSniff(sessionId);
      setSniffSession(session);
      setSniffQueue((current) => current ? { ...current, status: "stopped" } : current);
      setSniffRecoveryNeeded(session.phase === "restoration_required");
      setToast({
        kind: session.phase === "restoration_required" ? "error" : "success",
        message: session.message,
      });
    } catch (reason) {
      setToast({ kind: "error", message: readableError(reason) });
    }
  }

  async function handleRecoverSniff() {
    try {
      const result = await backend.recoverVideoSniffing();
      setSniffRecoveryNeeded(!result.recovered);
      if (result.recovered) {
        setSniffSession(null);
        setSniffQueue((current) => current ? { ...current, status: "stopped" } : current);
      }
      setToast({
        kind: result.recovered ? "success" : "error",
        message: result.message,
      });
    } catch (reason) {
      setSniffRecoveryNeeded(true);
      setToast({ kind: "error", message: readableError(reason) });
    }
  }

  function handleCancelSniffPlan() {
    setSniffPlan(null);
    setSniffQueue((current) => current?.status === "awaiting_authorization"
      ? { ...current, status: "stopped" }
      : current);
  }

  async function handleRevealOutput(path: string) {
    try {
      await backend.revealOutput(path);
    } catch (reason) {
      setToast({ kind: "error", message: `无法打开保存位置：${readableError(reason)}` });
    }
  }

  async function handleOpenExternal(url: string, label: string) {
    try {
      await backend.openExternal(url);
    } catch (reason) {
      setToast({ kind: "error", message: `无法打开${label}：${readableError(reason)}` });
    }
  }

  async function handleExportDiagnostics() {
    if (diagnosticsBusy) return;
    setDiagnosticsBusy(true);
    try {
      const destination = await backend.chooseDiagnosticDestination();
      if (!destination) return;
      const result = await backend.exportDiagnostics(destination);
      try {
        await backend.revealOutput(result.destination);
        setToast({
          kind: "success",
          message: `诊断日志已导出并在文件夹中显示：${result.destination}`,
        });
      } catch (reason) {
        setToast({
          kind: "warning",
          message: `诊断日志已导出到 ${result.destination}，但无法自动打开文件夹：${readableError(reason)}`,
        });
      }
    } catch (reason) {
      setToast({ kind: "error", message: `诊断日志导出失败：${readableError(reason)}` });
    } finally {
      setDiagnosticsBusy(false);
    }
  }

  async function handleBatchArticles() {
    const selectedArticles = tasks.filter(
      ({ task }) => task.kind === "article" && selectedTaskIds.has(task.id),
    );
    if (selectedArticles.length === 0) {
      setToast({ kind: "error", message: "请先勾选需要导出的公众号文章" });
      return;
    }
    const directory = await backend.chooseOutputDirectory();
    if (!directory) return;
    setBatchProgress({ current: 0, total: selectedArticles.length });
    const failedIds = new Set<number>();
    const completedIds = new Set<number>();
    const outputWarnings: string[] = [];
    let completed = 0;
    for (const summary of selectedArticles) {
      const taskId = summary.task.id;
      try {
        let detail: CaptureTaskDetail;
        try {
          detail = await backend.getTaskDetail(taskId);
        } catch {
          detail = summary;
        }
        if (!detail.article) {
          const processed = await processTask(taskId, true);
          if (!processed?.article) throw new Error("文章内容尚未读取");
          detail = processed;
        }
        setBusyTaskIds((current) => new Set(current).add(taskId));
        const result = await backend.exportArticle(taskId, directory, "pdf");
        if (result.warning) outputWarnings.push(result.warning);
        completed += 1;
        completedIds.add(taskId);
      } catch {
        failedIds.add(taskId);
      } finally {
        setBusyTaskIds((current) => withoutId(current, taskId));
        setBatchProgress({
          current: completed + failedIds.size,
          total: selectedArticles.length,
        });
      }
    }
    await reload(activeTaskId ?? undefined);
    setBatchProgress(null);
    setSelectedTaskIds((current) => {
      const next = new Set(current);
      for (const taskId of completedIds) next.delete(taskId);
      return next;
    });
    setToast({
      kind: failedIds.size ? "error" : outputWarnings.length ? "warning" : "success",
      message: failedIds.size || outputWarnings.length
        ? `公众号批量 PDF 完成：成功 ${completed} 篇，失败 ${failedIds.size} 篇${outputWarnings.length ? `；另有 ${outputWarnings.length} 项提示：${outputWarnings.join("；")}` : ""}`
        : `已批量导出 ${completed} 篇 PDF，文件保存在所选文件夹`,
    });
  }

  async function clearCompleted() {
    const completedIds = tasks.filter(({ task }) => task.status === "completed").map(({ task }) => task.id);
    if (completedIds.length === 0) {
      setToast({ kind: "success", message: "当前没有已完成任务需要清理" });
      return;
    }
    try {
      const cleared = await backend.clearTasks(completedIds);
      await reload();
      setToast({ kind: "success", message: `已清理 ${cleared} 项任务及内部缓存，本地导出文件已保留` });
    } catch (reason) {
      setToast({ kind: "error", message: readableError(reason) });
    }
  }

  async function clearSelected() {
    const taskIds = Array.from(selectedTaskIds);
    if (taskIds.length === 0) return;
    try {
      const cleared = await backend.clearTasks(taskIds);
      setSelectedTaskIds(new Set());
      await reload();
      setToast({ kind: "success", message: `已移除 ${cleared} 项并清掉内部缓存，本地导出文件已保留` });
    } catch (reason) {
      setToast({ kind: "error", message: readableError(reason) });
    }
  }

  if (isLoading) {
    return (
      <div className="app-loading">
        <img src={logoUrl} alt="讯栖" />
        <SpinnerGapIcon className="spin" size={24} />
        <p>{text("正在打开微信捕获队列…", "Opening the WeChat capture queue…")}</p>
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="app-loading app-error">
        <WarningCircleIcon size={42} weight="fill" />
        <h1>{text("没有读取到本地任务", "Local tasks could not be loaded")}</h1>
        <p>{localizeRuntimeMessage(loadError, language, "error")}</p>
        <button type="button" onClick={() => window.location.reload()}>{text("重新打开", "Reopen")}</button>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand-lockup">
          <img src={logoUrl} alt="" />
          <div className="brand-copy">
            <div className="brand-title-row">
              <strong>讯栖 <span lang="en">XunQi</span></strong>
              <b>栖</b>
            </div>
            <span>{text("讯来有迹，文止于栖。", "Messages traced, stories at rest.")}</span>
            <small>A QIDU Utility</small>
          </div>
        </div>
        <div className="listening-mark" aria-hidden="true">
          <span className="listening-line" />
          <img src={logoUrl} alt="" />
          <span className="listening-line" />
        </div>
        <div className="listening-status">
          <div className="listening-status-top">
            <button
              type="button"
              className="language-toggle"
              aria-label={language === "zh" ? "切换为英文" : "Switch to Chinese"}
              title={language === "zh" ? "切换为英文" : "Switch to Chinese"}
              onClick={() => setLanguage(language === "zh" ? "en" : "zh")}
            >
              <span aria-hidden="true">{language === "zh" ? "EN" : "中"}</span>
            </button>
            <button
              type="button"
              className="about-button"
              aria-label={text("关于", "About")}
              onClick={() => setShowAbout(true)}
            >
              <InfoIcon size={17} weight="bold" />
              <span>{text("关于", "About")}</span>
            </button>
            <button
              type="button"
              className="about-button"
              aria-label={diagnosticsBusy
                ? text("导出中", "Exporting")
                : text("导出日志", "Export Logs")}
              onClick={() => void handleExportDiagnostics()}
              disabled={diagnosticsBusy}
            >
              <FileArrowDownIcon size={17} weight="bold" />
              <span>{diagnosticsBusy
                ? text("导出中", "Exporting")
                : text("导出日志", "Export Logs")}</span>
            </button>
            <button type="button" className="link-guide-button" onClick={() => setShowLinkGuide(true)}>
              <QuestionIcon size={17} weight="bold" />
              {text("如何复制链接", "How to Copy Links")}
            </button>
            {sniffSession?.authorizationReusable && (
              <button
                type="button"
                className="sniff-active-pill"
                onClick={() => void handleStopSniff(sniffSession.sessionId)}
              >
                {text("连续授权已开 · 结束", "Continuous Access On · End")}
              </button>
            )}
            <strong><CheckCircleIcon size={18} weight="fill" />{text("微信监听中", "Listening to WeChat")}</strong>
          </div>
          <p>{localizeListeningActivity(listeningActivity, language)}</p>
        </div>
      </header>

      <div className="workspace">
        {!sidebarCollapsed ? (
          <TaskSidebar
            groups={groups}
            totalCount={tasks.length}
            articleCount={articleCount}
            videoCount={videoCount}
            query={query}
            filter={kindFilter}
            activeTaskId={activeTaskId}
            selectedTaskIds={selectedTaskIds}
            collapsedSources={collapsedSources}
            selectedCount={selectedTaskIds.size}
            selectedArticleCount={selectedArticleCount}
            selectedVideoCount={selectedVideoCount}
            visibleTaskCount={visibleTaskIds.length}
            allVisibleSelected={allVisibleSelected}
            batchBusy={
              batchProgress !== null
              || sniffQueue?.status === "running"
              || sniffQueue?.status === "awaiting_authorization"
              || Array.from(selectedTaskIds).some((taskId) => busyTaskIds.has(taskId))
            }
            onQueryChange={handleQueryChange}
            onFilterChange={setKindFilter}
            onActivate={setActiveTaskId}
            onToggleSelected={(taskId) =>
              setSelectedTaskIds((current) => toggleId(current, taskId))
            }
            onToggleVisibleSelection={() => setSelectedTaskIds((current) => {
              const next = new Set(current);
              if (allVisibleSelected) {
                for (const taskId of visibleTaskIds) next.delete(taskId);
              } else {
                for (const taskId of visibleTaskIds) next.add(taskId);
              }
              return next;
            })}
            onClearSelection={() => {
              setSelectedTaskIds(new Set());
              setSniffQueue(null);
            }}
            onToggleSource={(sourceName) =>
              setCollapsedSources((current) => toggleValue(current, sourceName))
            }
            onClearSelected={() => void clearSelected()}
            onClearCompleted={() => void clearCompleted()}
            onCollapse={() => setSidebarCollapsed(true)}
          />
        ) : (
          <button type="button" className="expand-sidebar" onClick={() => setSidebarCollapsed(false)} aria-label={text("展开任务列表", "Expand task list")}>
            <CaretRightIcon size={21} weight="bold" />
            <span>{tasks.length}</span>
          </button>
        )}
        <TaskDetail
          detail={activeDetail}
          busy={activeBusy}
          onProcess={(taskId) => void processTask(taskId)}
          onExport={(taskId, mode) => void handleExport(taskId, mode)}
          onDownload={(taskId, candidate) => void handleDownload(taskId, candidate)}
          onSniff={(taskId) => void handlePrepareSniff(taskId)}
          onStopSniff={(sessionId) => void handleStopSniff(sessionId)}
          onRecoverSniff={() => void handleRecoverSniff()}
          sniffSession={sniffSession}
          authorizedSniffSupported={authorizedSniffSupported}
          authorizedSniffExperimental={authorizedSniffExperimental}
          videoQualityMode={videoQualityMode}
          onVideoQualityModeChange={setVideoQualityMode}
          onOpenOriginal={(url) => void handleOpenExternal(url, "微信原文")}
          onRevealOutput={(path) => void handleRevealOutput(path)}
          articleBatch={{
            articleCount: selectedArticleCount,
            videoCount: selectedVideoCount,
            busy: batchProgress !== null,
            progressLabel: batchProgress ? `${batchProgress.current}/${batchProgress.total}` : null,
            onExport: () => void handleBatchArticles(),
            onClearSelection: () => setSelectedTaskIds(new Set()),
          }}
          videoQueue={{
            selectedCount: selectedSniffVideoCount,
            skippedCount: selectedSkippedVideoCount,
            running: sniffQueue?.status === "running" || sniffQueue?.status === "awaiting_authorization",
            progressLabel: sniffQueue ? formatQueueProgress(sniffQueue, language) : null,
            onStart: () => void handleStartVideoQueue(),
            onClearSelection: () => {
              setSelectedTaskIds(new Set());
              setSniffQueue(null);
            },
            qualityMode: videoQualityMode,
            onQualityModeChange: setVideoQualityMode,
            qualityLocked: sniffSession?.authorizationReusable === true,
          }}
        />
      </div>

      {toast && (
        <div className={`toast toast-${toast.kind}`} role="status" aria-live="polite">
          {toast.kind === "success" ? <CheckCircleIcon size={20} weight="fill" /> : <WarningCircleIcon size={20} weight="fill" />}
          <p>{localizeRuntimeMessage(toast.message, language, toast.kind)}</p>
          <button type="button" onClick={() => setToast(null)} aria-label={text("关闭提示", "Dismiss message")}><XIcon size={16} /></button>
        </div>
      )}
      {sniffRecoveryNeeded && (
        <div className="sniff-recovery-alert" role="alert">
          <WarningCircleIcon size={22} weight="fill" />
          <div>
            <strong>{text("授权助手的网络设置还没有恢复", "The authorized assistant has not restored network settings")}</strong>
            <p>{text(
              "恢复完成前不会启动新的嗅探任务。请先退出正在切换代理的其他软件，再执行恢复。",
              "New detection tasks stay blocked until recovery finishes. Close other apps that are changing proxy settings, then recover again.",
            )}</p>
          </div>
          <button type="button" onClick={() => void handleRecoverSniff()}>{text("立即恢复网络设置", "Restore Network Settings")}</button>
        </div>
      )}
      {sniffPlan && (
        <div className="modal-backdrop" role="presentation">
          <section className="authorization-dialog" role="dialog" aria-modal="true" aria-labelledby="sniff-dialog-title">
            <div className="authorization-dialog-icon"><LinkPermissionIcon /></div>
            <h2 id="sniff-dialog-title">{text("启用授权嗅探助手", "Enable Authorized Detection")}</h2>
            <div className="vpn-required-notice">
              {text("启用前请先关闭 VPN 或系统代理；关闭后再点下方按钮。", "Turn off your VPN or system proxy before enabling this feature.")}
            </div>
            <p>{authorizedSniffExperimental
              ? text(
                "这次操作会临时把系统 HTTP/HTTPS 流量接入本地下载助手，并把一张会话证书写入当前用户证书库。Windows 不使用指纹授权；根据系统或企业策略，首次可能出现证书信任确认。",
                "This temporarily routes system HTTP/HTTPS traffic through a local assistant and adds a session certificate to the current-user certificate store. Windows does not use fingerprint authorization; system or enterprise policy may show a certificate-trust confirmation on first use.",
              )
              : text(
                "这次操作会临时把系统 HTTP/HTTPS 流量接入本地下载助手，并安装一张仅供本次连续下载使用的证书。macOS 可能要求一次指纹或管理员认证。",
                "This temporarily routes system HTTP/HTTPS traffic through a local download assistant and installs a session certificate. macOS may request Touch ID or administrator approval once.",
              )}</p>
            <ul>
              <li>{text("只在你确认后启动；公开直链下载仍然优先。", "It starts only after your confirmation; public direct downloads remain the first choice.")}</li>
              <li>{text("启用期间，系统 HTTP/HTTPS 请求会先经过本地助手；它只对预设的微信/腾讯页面进行解析，其他流量只转发、不保存。", "While enabled, HTTP/HTTPS requests pass through the local assistant. It parses only predefined WeChat/Tencent pages and does not save other traffic.")}</li>
              <li>{text("匹配页面的 Cookie 与登录态仅在本机内存中临时经过助手，不显示、不写日志、不落盘、不上传。", "Matching-page cookies and session state pass through local memory only. They are not displayed, logged, saved, or uploaded.")}</li>
              <li>{text("因为会临时信任本地证书，这不是零风险功能；启用期间请暂停网银、密码修改等敏感操作。", "This is not risk-free because a local certificate is temporarily trusted. Avoid banking, password changes, and other sensitive activity while it is enabled.")}</li>
              <li>{text("只应用于你有权保存的内容；讯栖不会绕过账号权限或平台访问控制。", "Use it only for content you are allowed to save. XunQi does not bypass account permissions or platform access controls.")}</li>
              <li>{authorizedSniffExperimental
                ? text("视频下载完成后会保留本次会话，后续视频无需再次确认。", "The session remains active after a download, so later videos need no additional confirmation.")
                : text("视频下载完成后会保留授权，后续视频不再重复认证。", "Authorization remains available after a download so later videos do not request approval again.")}</li>
              <li>{text("你点击“结束并恢复网络”或退出讯栖时，才会恢复原代理并移除证书。", "The original proxy and certificate trust are restored when you choose End and Restore Network or quit XunQi.")}</li>
              <li>{text("VPN 与系统代理必须在启用前关闭；讯栖检测到它们时会直接阻止启动。", "VPN and system proxies must be off before starting; XunQi blocks activation when either is detected.")}</li>
              <li>{text("助手来源", "Assistant source")}：{sniffPlan.helperSource}；{text("微信改版后仍可能无法识别。", "future WeChat changes may still prevent detection.")}</li>
            </ul>
            <div className="authorization-dialog-warning">
              {text(
                "只有首次授权需要重新加载一次视频号子窗口，这是让微信使用新网络会话所必需的。讯栖会按已复制的分享链接自动下载原来那条；无需刷新，也不要转到浏览器扫码。之后连续下载不会再重新加载。",
                "Only the first authorization reloads the Channels subwindow so WeChat can use the new network session. XunQi follows the copied share link automatically; do not refresh or switch to browser QR login. Later downloads reuse the session.",
              )}
            </div>
            <div className="authorization-dialog-actions">
              <button type="button" className="secondary-action" onClick={handleCancelSniffPlan}>{text("取消", "Cancel")}</button>
              <button type="button" className="primary-action" onClick={() => void handleStartSniff()}>{text("同意并启用", "Agree and Enable")}</button>
            </div>
          </section>
        </div>
      )}
      {showAbout && (
        <BrandAboutDialog
          onClose={() => setShowAbout(false)}
          onExportDiagnostics={() => void handleExportDiagnostics()}
          diagnosticsBusy={diagnosticsBusy}
        />
      )}
      {showLinkGuide && (
        <div className="modal-backdrop" role="presentation">
          <section className="link-guide-dialog" role="dialog" aria-modal="true" aria-labelledby="link-guide-title">
            <header>
              <div>
                <span className="link-guide-icon"><QuestionIcon size={23} weight="duotone" /></span>
                <div>
                  <h2 id="link-guide-title">{text("如何复制微信链接", "How to Copy a WeChat Link")}</h2>
                  <p>{text("保持讯栖打开，复制后会自动进入左侧任务列表。", "Keep XunQi open. Copied links appear in the task list automatically.")}</p>
                </div>
              </div>
              <button type="button" onClick={() => setShowLinkGuide(false)} aria-label={text("关闭复制链接说明", "Close link guide")}>
                <XIcon size={18} />
              </button>
            </header>
            <div className="link-guide-options">
              <article>
                <span className="link-guide-kind"><ArticleIcon size={24} weight="duotone" /></span>
                <div>
                  <h3>{text("公众号文章", "Official Account Article")}</h3>
                  <p>{text("打开文章，点击右上角的三个点（有些微信版本显示四个点），再选择：", "Open the article, click the three-dot menu in the top-right (four dots in some versions), then choose:")}</p>
                  <span className="copy-link-action"><CopySimpleIcon size={17} />{text("复制链接", "Copy Link")}</span>
                </div>
              </article>
              <article>
                <span className="link-guide-kind"><VideoCameraIcon size={24} weight="duotone" /></span>
                <div>
                  <h3>{text("视频号", "WeChat Channels")}</h3>
                  <p>{text("打开视频，点击分享按钮，在弹出的菜单中选择：", "Open the video, click Share, then choose:")}</p>
                  <span className="copy-link-action"><ShareNetworkIcon size={17} />{text("复制链接", "Copy Link")}</span>
                </div>
              </article>
            </div>
            <div className="link-guide-note">
              {text("复制成功后不需要粘贴；讯栖检测到微信位于前台时会自动捕获新链接。", "No pasting is required. When WeChat is in the foreground, XunQi captures newly copied links automatically.")}
            </div>
            <div className="authorization-dialog-actions">
              <button type="button" className="primary-action" onClick={() => setShowLinkGuide(false)}>{text("知道了", "Got It")}</button>
            </div>
          </section>
        </div>
      )}
    </div>
  );
}

function LinkPermissionIcon() {
  return <LinkSimpleIcon size={26} weight="duotone" />;
}

function localizeListeningActivity(value: string, language: "zh" | "en") {
  if (language === "zh") return value;
  const exact: Record<string, string> = {
    "自动识别公众号与视频号，无需预先选择来源": "Automatically recognizes official-account and Channels links",
    "发现新的微信分享链接，正在自动分类…": "New WeChat share link found; classifying…",
    "视频已保存；连续授权仍可用于下一条": "Video saved; continuous authorization remains available",
  };
  if (exact[value]) return exact[value];
  const completed = value.match(/^连续下载完成：(\d+) 条视频已保存$/u);
  if (completed) return `Continuous download complete: ${completed[1]} videos saved`;
  const partial = value.match(/^连续下载结束：成功 (\d+) 条，失败 (\d+) 条$/u);
  if (partial) return `Continuous download finished: ${partial[1]} succeeded, ${partial[2]} failed`;
  return containsChinese(value) ? "WeChat capture status updated" : value;
}

function formatQueueProgress(queue: SniffQueueState, language: "zh" | "en") {
  if (queue.status !== "completed") {
    return `${Math.min(queue.currentIndex + 1, queue.taskIds.length)}/${queue.taskIds.length}`;
  }
  if (language === "en") {
    return `Complete ${queue.taskIds.length}/${queue.taskIds.length}${queue.failedCount ? ` (${queue.failedCount} failed)` : ""}`;
  }
  return `队列完成 ${queue.taskIds.length}/${queue.taskIds.length}${queue.failedCount ? `（失败 ${queue.failedCount}）` : ""}`;
}

function localizeRuntimeMessage(
  value: string,
  language: "zh" | "en",
  kind: "success" | "warning" | "error",
) {
  if (language === "zh" || !containsChinese(value)) return value;
  const exact: Record<string, string> = {
    "这条微信分享链接已经收取过": "This WeChat share link is already in the task list.",
    "微信分享链接已进入捕获任务": "The WeChat share link was added to the capture queue.",
    "勾选的视频里没有需要授权嗅探的任务": "None of the selected videos require authorized detection.",
    "当前没有已完成任务需要清理": "There are no completed tasks to clear.",
  };
  if (exact[value]) return exact[value];

  const savedPdf = value.match(/^原版 PDF 已保存到 (.+)$/u);
  if (savedPdf) return `Original PDF saved to ${savedPdf[1]}`;
  const exportedArticle = value.match(/^文章已导出到 (.+)$/u);
  if (exportedArticle) return `Article exported to ${exportedArticle[1]}`;
  const downloadedVideo = value.match(/^视频已下载到 (.+)$/u);
  if (downloadedVideo) return `Video downloaded to ${downloadedVideo[1]}`;
  const cleared = value.match(/^已清理 (\d+) 项/u);
  if (cleared) return `${cleared[1]} tasks and their internal cache were cleared. Local exports were kept.`;
  const removed = value.match(/^已移除 (\d+) 项/u);
  if (removed) return `${removed[1]} tasks and their internal cache were removed. Local exports were kept.`;

  if (kind === "success") return "The operation completed successfully.";
  if (kind === "warning") return "The operation completed with a warning. Switch to Chinese or export diagnostics for the original details.";
  return "The operation failed. Switch to Chinese or export diagnostics for the original details.";
}

function containsChinese(value: string) {
  return /[\u3400-\u9fff]/u.test(value);
}

function sniffConflictMessage(
  conflict: SniffAuthorizationPlan["conflict"],
  language: "zh" | "en",
  chineseFallback: string,
  englishFallback: string,
) {
  if (language === "zh") return conflict?.message ?? chineseFallback;
  if (conflict?.code === "platform_sniffer_unavailable") {
    return "Authorized detection is not available on this operating system. Article export and public direct-video downloads still work.";
  }
  return conflict?.message && !containsChinese(conflict.message)
    ? conflict.message
    : englishFallback;
}

function normalizeSearch(value: string) {
  return value.trim().toLocaleLowerCase("zh-CN").replace(/\s+/g, " ");
}

function detectPlatform(): "macos" | "windows" | "other" {
  const userAgent = navigator.userAgent.toLocaleLowerCase("en-US");
  if (userAgent.includes("windows")) return "windows";
  if (userAgent.includes("macintosh") || userAgent.includes("mac os")) return "macos";
  return isTauriRuntime() ? "other" : "macos";
}

function isWechatShareLink(value: string) {
  const trimmed = value.trim();
  if (!trimmed || /\s/.test(trimmed)) return false;
  try {
    const url = new URL(trimmed);
    if (url.protocol !== "https:") return false;
    const host = url.hostname.toLocaleLowerCase("en-US");
    if (host === "mp.weixin.qq.com") {
      return /^\/s\/[^/]+$/.test(url.pathname)
        || (url.pathname === "/s" && url.searchParams.has("sn"));
    }
    if (host === "channels.weixin.qq.com") {
      return url.pathname === "/finder-preview/pages/sph" && url.searchParams.has("id");
    }
    if (host === "weixin.qq.com") {
      return /^\/sph\/[^/]+$/.test(url.pathname);
    }
    return host === "finder.video.qq.com";
  } catch {
    return false;
  }
}

function readableError(reason: unknown) {
  return reason instanceof Error ? reason.message : String(reason);
}

function toggleId(values: Set<number>, id: number) {
  const next = new Set(values);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  return next;
}

function toggleValue(values: Set<string>, value: string) {
  const next = new Set(values);
  if (next.has(value)) next.delete(value);
  else next.add(value);
  return next;
}

function withoutId(values: Set<number>, id: number) {
  const next = new Set(values);
  next.delete(id);
  return next;
}

function mergeTaskSummaries(
  current: CaptureTaskDetail[],
  summaries: CaptureTaskDetail[],
) {
  const currentById = new Map(current.map((detail) => [detail.task.id, detail]));
  return summaries.map((summary) => {
    const existing = currentById.get(summary.task.id);
    return existing
      ? { ...summary, article: existing.article, video: existing.video }
      : summary;
  });
}

async function clipboardFingerprint(value: string) {
  const digest = await crypto.subtle.digest("SHA-256", new TextEncoder().encode(value));
  return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
}

function isActiveSniffPhase(phase: SniffSessionSnapshot["phase"]) {
  return ["starting", "awaiting_playback", "capturing", "saving", "restoring"].includes(phase);
}

function needsAuthorizedSniffDownload({ task, video }: CaptureTaskDetail) {
  return task.kind === "video"
    && task.status !== "completed"
    && !task.completedPath
    && video !== null
    && video.candidates.every((candidate) => !candidate.downloadable);
}

function App(props: AppProps) {
  return (
    <LanguageProvider>
      <AppContent {...props} />
    </LanguageProvider>
  );
}

export default App;
