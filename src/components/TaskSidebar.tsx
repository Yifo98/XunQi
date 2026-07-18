import { useState } from "react";
import {
  CaretDownIcon,
  CaretRightIcon,
  CaretLeftIcon,
  FileTextIcon,
  FunnelSimpleIcon,
  MagnifyingGlassIcon,
  TrashIcon,
  VideoCameraIcon,
} from "@phosphor-icons/react";
import type { CaptureKind, CaptureTaskDetail } from "../lib/backend";
import { useI18n, type AppLanguage } from "../i18n";

export type KindFilter = "all" | CaptureKind;

type TaskGroup = {
  sourceName: string;
  tasks: CaptureTaskDetail[];
};

type TaskSidebarProps = {
  groups: TaskGroup[];
  totalCount: number;
  articleCount: number;
  videoCount: number;
  query: string;
  filter: KindFilter;
  activeTaskId: number | null;
  selectedTaskIds: Set<number>;
  collapsedSources: Set<string>;
  selectedCount: number;
  selectedArticleCount: number;
  selectedVideoCount: number;
  visibleTaskCount: number;
  allVisibleSelected: boolean;
  batchBusy: boolean;
  onQueryChange: (value: string) => void;
  onFilterChange: (filter: KindFilter) => void;
  onActivate: (taskId: number) => void;
  onToggleSelected: (taskId: number) => void;
  onToggleVisibleSelection: () => void;
  onClearSelection: () => void;
  onToggleSource: (sourceName: string) => void;
  onClearSelected: () => void;
  onClearCompleted: () => void;
  onCollapse: () => void;
};

export function TaskSidebar({
  groups,
  totalCount,
  articleCount,
  videoCount,
  query,
  filter,
  activeTaskId,
  selectedTaskIds,
  collapsedSources,
  selectedCount,
  selectedArticleCount,
  selectedVideoCount,
  visibleTaskCount,
  allVisibleSelected,
  batchBusy,
  onQueryChange,
  onFilterChange,
  onActivate,
  onToggleSelected,
  onToggleVisibleSelection,
  onClearSelection,
  onToggleSource,
  onClearSelected,
  onClearCompleted,
  onCollapse,
}: TaskSidebarProps) {
  const { language, text } = useI18n();
  const [expandedSources, setExpandedSources] = useState<Set<string>>(new Set());
  const visibleSelectedCount = groups.reduce(
    (count, group) => count + group.tasks.filter(({ task }) => selectedTaskIds.has(task.id)).length,
    0,
  );
  const hasHiddenSelection = selectedCount > visibleSelectedCount;

  return (
    <aside className="task-sidebar" aria-label={text("捕获任务", "Capture Tasks")}>
      <div className="sidebar-heading">
        <div>
          <strong>{text("捕获任务", "Capture Tasks")}</strong>
          <span>{totalCount}</span>
        </div>
        <button type="button" className="icon-button" onClick={onCollapse} aria-label={text("收起任务列表", "Collapse task list")}>
          <CaretLeftIcon size={20} weight="bold" />
        </button>
      </div>

      <div className="task-search-row">
        <label className="task-search">
          <MagnifyingGlassIcon size={18} />
          <input
            value={query}
            onChange={(event) => onQueryChange(event.target.value)}
            placeholder={text("搜索公众号、视频号或标题", "Search account, channel, or title")}
            aria-label={text("搜索公众号、视频号或标题", "Search account, channel, or title")}
          />
        </label>
        <span className="filter-icon" aria-hidden="true" title={text("使用下方标签筛选", "Filter with the tabs below")}>
          <FunnelSimpleIcon size={19} />
        </span>
      </div>

      <div className="kind-tabs" role="tablist" aria-label={text("任务类型", "Task type")}>
        <FilterTab selected={filter === "all"} onClick={() => onFilterChange("all")}>
          {text("全部", "All")} <span>{totalCount}</span>
        </FilterTab>
        <FilterTab selected={filter === "article"} onClick={() => onFilterChange("article")}>
          {text("公众号", "Articles")} <span>{articleCount}</span>
        </FilterTab>
        <FilterTab selected={filter === "video"} onClick={() => onFilterChange("video")}>
          {text("视频号", "Channels")} <span>{videoCount}</span>
        </FilterTab>
      </div>

      {visibleTaskCount > 0 && (
        <div className="bulk-selection-row">
          <button
            type="button"
            className={allVisibleSelected ? "bulk-selection-active" : ""}
            aria-pressed={allVisibleSelected}
            onClick={onToggleVisibleSelection}
            disabled={batchBusy}
          >
            <span className="bulk-selection-mark" aria-hidden="true">{allVisibleSelected ? "✓" : ""}</span>
            {allVisibleSelected
              ? language === "zh"
                ? `${hasHiddenSelection ? "取消当前" : "取消全选"}${filterLabel(filter, language)} ${visibleTaskCount} 项`
                : `${hasHiddenSelection ? "Deselect Current" : "Deselect All"}${englishFilterSuffix(filter)} (${visibleTaskCount})`
              : language === "zh"
                ? `全选${filterLabel(filter, language)} ${visibleTaskCount} 项`
                : `Select All${englishFilterSuffix(filter)} (${visibleTaskCount})`}
          </button>
          {hasHiddenSelection ? (
            <button
              type="button"
              className="bulk-clear-all"
              onClick={onClearSelection}
              disabled={batchBusy}
            >
              {text(`取消全部已选 ${selectedCount} 项`, `Deselect All (${selectedCount})`)}
            </button>
          ) : (
            <small>{text("切换分类后仍保留已选项", "Selections remain when switching tabs")}</small>
          )}
        </div>
      )}

      <div className="task-groups">
        {groups.length === 0 ? (
          <div className="sidebar-empty">
            <MagnifyingGlassIcon size={30} />
            <strong>{totalCount === 0
              ? text("还没有捕获任务", "No captured tasks yet")
              : text("没有匹配的任务", "No matching tasks")}</strong>
            <p>{totalCount === 0
              ? text("去微信复制文章或视频号分享链接。", "Copy an article or Channels share link in WeChat.")
              : text("换个关键词或清除筛选。", "Try another keyword or clear the filter.")}</p>
          </div>
        ) : (
          groups.map((group) => {
            const collapsed = collapsedSources.has(group.sourceName);
            const expanded = expandedSources.has(group.sourceName);
            const visibleTasks = expanded ? group.tasks : group.tasks.slice(0, 3);
            return (
              <section className="task-group" key={group.sourceName}>
                <button
                  type="button"
                  className="task-group-heading"
                  onClick={() => onToggleSource(group.sourceName)}
                  aria-expanded={!collapsed}
                >
                  <span className="group-caret">
                    {collapsed ? <CaretRightIcon size={15} /> : <CaretDownIcon size={15} />}
                  </span>
                  <strong>{group.sourceName}</strong>
                  <span>{group.tasks.length}</span>
                </button>
                {!collapsed && (
                  <div className="task-rows">
                    {visibleTasks.map(({ task }) => (
                      <div
                        className={`task-row ${activeTaskId === task.id ? "task-row-active" : ""}`}
                        key={task.id}
                      >
                        <label className="task-checkbox" onClick={(event) => event.stopPropagation()}>
                          <input
                            type="checkbox"
                            checked={selectedTaskIds.has(task.id)}
                            onChange={() => onToggleSelected(task.id)}
                            aria-label={language === "zh" ? `选择${task.title}` : `Select ${task.title}`}
                          />
                        </label>
                        <button type="button" className="task-row-main" onClick={() => onActivate(task.id)}>
                          <span className="task-kind-icon" aria-hidden="true">
                            {task.kind === "article" ? (
                              <FileTextIcon size={20} />
                            ) : (
                              <VideoCameraIcon size={20} />
                            )}
                          </span>
                          <span className="task-row-copy">
                            <span className="task-title-line">
                              <strong>{task.title}</strong>
                              <time dateTime={task.createdAt}>{formatTaskTime(task.createdAt, language)}</time>
                            </span>
                            <TaskStatus kind={task.kind} status={task.status} />
                          </span>
                        </button>
                      </div>
                    ))}
                    {group.tasks.length > 3 && (
                      <button
                        type="button"
                        className="show-more-tasks"
                        onClick={() => setExpandedSources((current) => toggleSource(current, group.sourceName))}
                      >
                        {expanded
                          ? text("收起", "Show Less")
                          : text(`查看更多 (${group.tasks.length - 3})`, `Show More (${group.tasks.length - 3})`)}
                        <CaretDownIcon className={expanded ? "show-more-open" : ""} size={14} />
                      </button>
                    )}
                  </div>
                )}
              </section>
            );
          })
        )}
      </div>

      <div className="sidebar-footer">
        {selectedCount > 0 ? (
          <div className="batch-bar">
            <div>
              <strong>{text(`已选 ${selectedCount} 项`, `${selectedCount} Selected`)}</strong>
              <small>
                {text("公众号", "Articles")} {selectedArticleCount}
                {selectedVideoCount > 0 ? ` · ${text("视频号", "Channels")} ${selectedVideoCount}` : ""}
              </small>
            </div>
            <div className="batch-actions">
              <button type="button" className="remove-selected" onClick={onClearSelected} disabled={batchBusy}>
                <TrashIcon size={16} />
                {text("移除并清缓存", "Remove and Clear Cache")}
              </button>
            </div>
          </div>
        ) : (
          <button type="button" className="clear-completed" onClick={onClearCompleted}>
            <TrashIcon size={17} />
            {text("清空已完成缓存", "Clear Completed Cache")}
          </button>
        )}
        <p>{text("内容留在本地", "Content stays local")} · A QIDU Utility</p>
      </div>
    </aside>
  );
}

function filterLabel(filter: KindFilter, language: AppLanguage) {
  const labels = language === "zh"
    ? { all: "全部", article: "公众号", video: "视频号" }
    : { all: "Tasks", article: "Articles", video: "Channels" };
  return labels[filter];
}

function englishFilterSuffix(filter: KindFilter) {
  return filter === "all" ? "" : ` ${filterLabel(filter, "en")}`;
}

function toggleSource(values: Set<string>, sourceName: string) {
  const next = new Set(values);
  if (next.has(sourceName)) next.delete(sourceName);
  else next.add(sourceName);
  return next;
}

function FilterTab({
  selected,
  onClick,
  children,
}: {
  selected: boolean;
  onClick: () => void;
  children: React.ReactNode;
}) {
  return (
    <button type="button" role="tab" aria-selected={selected} className={selected ? "active" : ""} onClick={onClick}>
      {children}
    </button>
  );
}

function TaskStatus({ kind, status }: Pick<CaptureTaskDetail["task"], "kind" | "status">) {
  const { language } = useI18n();
  const label = language === "zh" ? {
    queued: "等待处理",
    processing: kind === "video" ? "正在识别" : "正在读取",
    ready: kind === "video" ? "可下载" : "可导出",
    needs_attention: "需查看",
    exporting: "正在导出",
    downloading: "下载中",
    completed: "已完成",
    failed: "失败",
  }[status] : {
    queued: "Queued",
    processing: kind === "video" ? "Detecting" : "Reading",
    ready: kind === "video" ? "Downloadable" : "Exportable",
    needs_attention: "Review",
    exporting: "Exporting",
    downloading: "Downloading",
    completed: "Completed",
    failed: "Failed",
  }[status];
  return <span className={`row-status row-status-${status}`}>{label}</span>;
}

function formatTaskTime(value: string, language: AppLanguage) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return language === "zh" ? "刚刚" : "Just now";
  return new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en-US", {
    hour: "2-digit",
    minute: "2-digit",
    hour12: false,
  }).format(date);
}
