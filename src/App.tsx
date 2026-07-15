import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  ArticleIcon,
  CaretRightIcon,
  CheckCircleIcon,
  CopySimpleIcon,
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
import { TaskDetail } from "./components/TaskDetail";
import { TaskSidebar, type KindFilter } from "./components/TaskSidebar";
import {
  isTauriRuntime,
  tauriBackend,
  type ArticleExportMode,
  type Backend,
  type CaptureTaskDetail,
  type DetectedVideo,
  type SniffAuthorizationPlan,
  type SniffSessionSnapshot,
} from "./lib/backend";
import { createPreviewBackend } from "./lib/previewBackend";

type AppProps = {
  backend?: Backend;
};

type SniffQueueState = {
  taskIds: number[];
  currentIndex: number;
  destinationDirectory: string;
  status: "awaiting_authorization" | "running" | "completed" | "stopped";
  failedCount: number;
};

const defaultBackend = isTauriRuntime() ? tauriBackend : createPreviewBackend();

function App({ backend = defaultBackend }: AppProps) {
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
  const [sniffRecoveryNeeded, setSniffRecoveryNeeded] = useState(false);
  const [showLinkGuide, setShowLinkGuide] = useState(false);
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
    if (!["completed", "failed_reusable"].includes(sniffSession.phase)) return;
    if (sniffQueue.taskIds[sniffQueue.currentIndex] !== sniffSession.taskId) return;
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
          setToast({ kind: "error", message: plan.conflict?.message ?? "队列中的下一条无法启动" });
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
        );
        setSniffSession(session);
        setSniffQueue({ ...nextQueue, status: "running" });
      }).catch((reason) => {
        setSniffQueue({ ...nextQueue, status: "stopped" });
        setToast({ kind: "error", message: readableError(reason) });
      });
    }, 0);
    return () => window.clearTimeout(advanceTimer);
  }, [backend, reload, sniffQueue, sniffSession]);

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
    setSelectedTaskIds(new Set());
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
  const selectedSniffVideoCount = tasks.filter(
    ({ task, video }) =>
      task.kind === "video"
      && selectedTaskIds.has(task.id)
      && video !== null
      && video.candidates.every((candidate) => !candidate.downloadable),
  ).length;
  const activeBusy = activeTaskId !== null && busyTaskIds.has(activeTaskId);

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
          message: plan.conflict?.message ?? "当前不能安全启用授权嗅探助手，可稍后重试或打开微信原文。",
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
      const session = await backend.startVideoSniff(plan.taskId, plan.planId, directory);
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
      .filter(({ task, video }) => (
        task.kind === "video"
        && selectedTaskIds.has(task.id)
        && video !== null
        && video.candidates.every((candidate) => !candidate.downloadable)
      ))
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
          message: plan.conflict?.message ?? "当前不能启动连续下载队列",
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
        <p>正在打开微信捕获队列…</p>
      </div>
    );
  }

  if (loadError) {
    return (
      <div className="app-loading app-error">
        <WarningCircleIcon size={42} weight="fill" />
        <h1>没有读取到本地任务</h1>
        <p>{loadError}</p>
        <button type="button" onClick={() => window.location.reload()}>重新打开</button>
      </div>
    );
  }

  return (
    <div className="app-shell">
      <header className="app-header">
        <div className="brand-lockup">
          <img src={logoUrl} alt="" />
          <div className="brand-copy">
            <strong>讯栖</strong>
            <span>讯来有迹，文止于栖。</span>
            <small>讯者，消息之所至；栖者，文章之所安。</small>
          </div>
        </div>
        <div className="listening-mark" aria-hidden="true">
          <span className="listening-line" />
          <img src={logoUrl} alt="" />
          <span className="listening-line" />
        </div>
        <div className="listening-status">
          <div className="listening-status-top">
            <button type="button" className="link-guide-button" onClick={() => setShowLinkGuide(true)}>
              <QuestionIcon size={17} weight="bold" />
              如何复制链接
            </button>
            {sniffSession?.authorizationReusable && (
              <button
                type="button"
                className="sniff-active-pill"
                onClick={() => void handleStopSniff(sniffSession.sessionId)}
              >
                连续授权已开 · 结束
              </button>
            )}
            <strong><CheckCircleIcon size={18} weight="fill" />微信监听中</strong>
          </div>
          <p>{listeningActivity}</p>
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
            batchBusy={
              batchProgress !== null
              || sniffQueue?.status === "running"
              || sniffQueue?.status === "awaiting_authorization"
              || Array.from(selectedTaskIds).some((taskId) => busyTaskIds.has(taskId))
            }
            onQueryChange={handleQueryChange}
            onFilterChange={(value) => {
              setKindFilter(value);
              setSelectedTaskIds(new Set());
            }}
            onActivate={setActiveTaskId}
            onToggleSelected={(taskId) =>
              setSelectedTaskIds((current) => toggleId(current, taskId))
            }
            onToggleSource={(sourceName) =>
              setCollapsedSources((current) => toggleValue(current, sourceName))
            }
            onClearSelected={() => void clearSelected()}
            onClearCompleted={() => void clearCompleted()}
            onCollapse={() => setSidebarCollapsed(true)}
          />
        ) : (
          <button type="button" className="expand-sidebar" onClick={() => setSidebarCollapsed(false)} aria-label="展开任务列表">
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
            running: sniffQueue?.status === "running" || sniffQueue?.status === "awaiting_authorization",
            progressLabel: sniffQueue
              ? sniffQueue.status === "completed"
                ? `队列完成 ${sniffQueue.taskIds.length}/${sniffQueue.taskIds.length}${sniffQueue.failedCount ? `（失败 ${sniffQueue.failedCount}）` : ""}`
                : `${Math.min(sniffQueue.currentIndex + 1, sniffQueue.taskIds.length)}/${sniffQueue.taskIds.length}`
              : null,
            onStart: () => void handleStartVideoQueue(),
            onClearSelection: () => {
              setSelectedTaskIds(new Set());
              setSniffQueue(null);
            },
          }}
        />
      </div>

      {toast && (
        <div className={`toast toast-${toast.kind}`} role="status" aria-live="polite">
          {toast.kind === "success" ? <CheckCircleIcon size={20} weight="fill" /> : <WarningCircleIcon size={20} weight="fill" />}
          <p>{toast.message}</p>
          <button type="button" onClick={() => setToast(null)} aria-label="关闭提示"><XIcon size={16} /></button>
        </div>
      )}
      {sniffRecoveryNeeded && (
        <div className="sniff-recovery-alert" role="alert">
          <WarningCircleIcon size={22} weight="fill" />
          <div>
            <strong>授权助手的网络设置还没有恢复</strong>
            <p>恢复完成前不会启动新的嗅探任务。请先退出正在切换代理的其他软件，再执行恢复。</p>
          </div>
          <button type="button" onClick={() => void handleRecoverSniff()}>立即恢复网络设置</button>
        </div>
      )}
      {sniffPlan && (
        <div className="modal-backdrop" role="presentation">
          <section className="authorization-dialog" role="dialog" aria-modal="true" aria-labelledby="sniff-dialog-title">
            <div className="authorization-dialog-icon"><LinkPermissionIcon /></div>
            <h2 id="sniff-dialog-title">启用授权嗅探助手</h2>
            <div className="vpn-required-notice">
              启用前请先关闭 VPN 或系统代理；关闭后再点下方按钮。
            </div>
            <p>这次操作会临时把系统 HTTP/HTTPS 流量接入本地下载助手，并安装一张仅供本次连续下载使用的证书。系统可能要求一次指纹或管理员认证。</p>
            <ul>
              <li>只在你确认后启动；公开直链下载仍然优先。</li>
              <li>启用期间，系统 HTTP/HTTPS 请求会先经过本地助手；它只对预设的微信/腾讯页面进行解析，其他流量只转发、不保存。</li>
              <li>匹配页面的 Cookie 与登录态仅在本机内存中临时经过助手，不显示、不写日志、不落盘、不上传。</li>
              <li>因为会临时信任本地证书，这不是零风险功能；启用期间请暂停网银、密码修改等敏感操作。</li>
              <li>只应用于你有权保存的内容；讯栖不会绕过账号权限或平台访问控制。</li>
              <li>视频下载完成后会保留授权，后续视频不再重复认证。</li>
              <li>你点击“结束并恢复网络”或退出讯栖时，才会恢复原代理并移除证书。</li>
              <li>VPN 与系统代理必须在启用前关闭；讯栖检测到它们时会直接阻止启动。</li>
              <li>助手来源：{sniffPlan.helperSource}；微信改版后仍可能无法识别。</li>
            </ul>
            <div className="authorization-dialog-warning">
              只有首次授权需要重新加载一次视频号子窗口，这是让微信使用新网络会话所必需的。讯栖会按已复制的分享链接自动下载原来那条；无需刷新，也不要转到浏览器扫码。之后连续下载不会再重新加载。
            </div>
            <div className="authorization-dialog-actions">
              <button type="button" className="secondary-action" onClick={handleCancelSniffPlan}>取消</button>
              <button type="button" className="primary-action" onClick={() => void handleStartSniff()}>同意并启用</button>
            </div>
          </section>
        </div>
      )}
      {showLinkGuide && (
        <div className="modal-backdrop" role="presentation">
          <section className="link-guide-dialog" role="dialog" aria-modal="true" aria-labelledby="link-guide-title">
            <header>
              <div>
                <span className="link-guide-icon"><QuestionIcon size={23} weight="duotone" /></span>
                <div>
                  <h2 id="link-guide-title">如何复制微信链接</h2>
                  <p>保持讯栖打开，复制后会自动进入左侧任务列表。</p>
                </div>
              </div>
              <button type="button" onClick={() => setShowLinkGuide(false)} aria-label="关闭复制链接说明">
                <XIcon size={18} />
              </button>
            </header>
            <div className="link-guide-options">
              <article>
                <span className="link-guide-kind"><ArticleIcon size={24} weight="duotone" /></span>
                <div>
                  <h3>公众号文章</h3>
                  <p>打开文章，点击右上角的三个点（有些微信版本显示四个点），再选择：</p>
                  <span className="copy-link-action"><CopySimpleIcon size={17} />复制链接</span>
                </div>
              </article>
              <article>
                <span className="link-guide-kind"><VideoCameraIcon size={24} weight="duotone" /></span>
                <div>
                  <h3>视频号</h3>
                  <p>打开视频，点击分享按钮，在弹出的菜单中选择：</p>
                  <span className="copy-link-action"><ShareNetworkIcon size={17} />复制链接</span>
                </div>
              </article>
            </div>
            <div className="link-guide-note">
              复制成功后不需要粘贴；讯栖检测到微信位于前台时会自动捕获新链接。
            </div>
            <div className="authorization-dialog-actions">
              <button type="button" className="primary-action" onClick={() => setShowLinkGuide(false)}>知道了</button>
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

function normalizeSearch(value: string) {
  return value.trim().toLocaleLowerCase("zh-CN").replace(/\s+/g, " ");
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

export default App;
